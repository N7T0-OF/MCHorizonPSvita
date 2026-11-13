//! The PC bridge: a small TCP server inside the application, for development.
//!
//! It lets a PC on the same network read what the console really does (state, log,
//! the picture on screen), press buttons, move the camera and send files into the
//! application's data folder, without photographs or cable swaps. It is off until the
//! user turns it on in the menu, and it reaches nothing outside the data folder.
//!
//! Protocol: one command per line; the answer is `OK <length>\n` and that many bytes,
//! or `ERR <message>\n`. The client is `vita/bridge` (`vita-bridge`).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const PORT: u16 = 4747;
const MAX_UPLOAD: u64 = 256 << 20;

/// What only the frame loop can answer.
pub enum Request {
    State,
    /// The picture on screen, as rows of RGBA.
    Shot,
    /// A button, as if pressed for one frame.
    Key(String),
    /// x, y, z, yaw, pitch of the 3D camera.
    Camera([f32; 5]),
    Set(String, i32),
    /// Forget the loaded scene, so that JOUER loads it again from the card.
    Reload,
    /// Start a named test.
    Run(String),
    Quit,
}

pub struct Envelope {
    pub request: Request,
    pub reply: Sender<Result<Vec<u8>, String>>,
}

pub struct Bridge {
    /// Commands are refused while this is false.
    pub enabled: Arc<AtomicBool>,
    /// What the listener says about itself, for the menu.
    pub status: Arc<Mutex<String>>,
    requests: Receiver<Envelope>,
}

impl Bridge {
    /// Starts the listener thread. `root` is the only folder `get` and `put` reach.
    pub fn start(root: String, version: &'static str) -> Self {
        let enabled = Arc::new(AtomicBool::new(true));
        let status = Arc::new(Mutex::new("démarrage".to_owned()));
        let (sender, requests) = mpsc::channel();
        let (thread_enabled, thread_status) = (enabled.clone(), status.clone());
        let spawned = std::thread::Builder::new().name("pc-bridge".into()).stack_size(256 * 1024).spawn(move || {
            // Off the console (the PC test of this protocol) only this machine may connect.
            let host = if cfg!(target_os = "vita") { "0.0.0.0" } else { "127.0.0.1" };
            let listener = match TcpListener::bind((host, PORT)) {
                Ok(listener) => listener,
                Err(error) => {
                    *thread_status.lock().unwrap() = format!("écoute impossible : {error}");
                    return;
                }
            };
            *thread_status.lock().unwrap() = format!("écoute sur le port {PORT}");
            for stream in listener.incoming().flatten() {
                // One client at a time: this is a development tool, not a service.
                let _ = serve(&stream, &root, version, &thread_enabled, &sender);
            }
        });
        if let Err(error) = spawned {
            *status.lock().unwrap() = format!("thread impossible : {error}");
        }
        Bridge { enabled, status, requests }
    }

    pub fn poll(&self) -> Option<Envelope> {
        self.requests.try_recv().ok()
    }
}

/// A path under `root`, or why `name` is refused.
fn resolve(root: &str, name: &str) -> Result<PathBuf, String> {
    let plain = !name.is_empty()
        && !name.starts_with('/')
        && !name.split('/').any(|part| part.is_empty() || part == "." || part == "..")
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b));
    if !plain {
        return Err(format!("refused path {name:?}"));
    }
    Ok(PathBuf::from(format!("{root}/{name}")))
}

fn serve(stream: &TcpStream, root: &str, version: &str, enabled: &AtomicBool, sender: &Sender<Envelope>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let mut words = line.split_whitespace();
        let command = words.next().unwrap_or("");
        let ask = |request: Request| -> Result<Vec<u8>, String> {
            let (reply, answer) = mpsc::channel();
            sender.send(Envelope { request, reply }).map_err(|_| "the application is closing".to_owned())?;
            answer.recv_timeout(Duration::from_secs(20)).map_err(|_| "the frame loop did not answer".to_owned())?
        };
        let result = if !enabled.load(Relaxed) {
            Err("the bridge is turned off on the console".to_owned())
        } else {
            match command {
                "ping" => Ok(format!("pong {version}").into_bytes()),
                "state" => ask(Request::State),
                "shot" => ask(Request::Shot),
                "reload" => ask(Request::Reload),
                "quit" => ask(Request::Quit),
                "run" => match words.next() {
                    Some(name) => ask(Request::Run(name.to_owned())),
                    None => Err("run <test>".to_owned()),
                },
                "key" => match words.next() {
                    Some(name) => ask(Request::Key(name.to_owned())),
                    None => Err("key <name>".to_owned()),
                },
                "cam" => {
                    let values: Vec<f32> = words.by_ref().filter_map(|word| word.parse().ok()).collect();
                    match <[f32; 5]>::try_from(values) {
                        Ok(camera) if camera.iter().all(|v| v.is_finite()) => ask(Request::Camera(camera)),
                        _ => Err("cam <x> <y> <z> <yaw> <pitch>".to_owned()),
                    }
                }
                "set" => match (words.next(), words.next().and_then(|word| word.parse().ok())) {
                    (Some(name), Some(value)) => ask(Request::Set(name.to_owned(), value)),
                    _ => Err("set <name> <integer>".to_owned()),
                },
                "get" => match words.next() {
                    Some(name) => resolve(root, name).and_then(|path| std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))),
                    None => Err("get <path>".to_owned()),
                },
                "put" => match (words.next(), words.next().and_then(|word| word.parse::<u64>().ok())) {
                    (Some(name), Some(length)) if length <= MAX_UPLOAD => {
                        // The bytes follow the line whatever happens next: read them all,
                        // so a refused upload cannot be mistaken for commands.
                        let target = resolve(root, name);
                        let written = receive(&mut reader, length, target.as_ref().ok())?;
                        target.and(written).map(|()| format!("{length} bytes").into_bytes())
                    }
                    _ => Err(format!("put <path> <length up to {MAX_UPLOAD}>")),
                },
                other => Err(format!("unknown command {other:?}")),
            }
        };
        let mut out = stream;
        match result {
            Ok(bytes) => {
                out.write_all(format!("OK {}\n", bytes.len()).as_bytes())?;
                out.write_all(&bytes)?;
            }
            Err(message) => out.write_all(format!("ERR {}\n", message.replace('\n', " ")).as_bytes())?,
        }
        out.flush()?;
    }
}

/// Reads `length` bytes of an upload, into `target` when there is one: first beside
/// it, so a broken transfer cannot leave half a file under the real name.
fn receive(reader: &mut impl Read, length: u64, target: Option<&PathBuf>) -> std::io::Result<Result<(), String>> {
    let partial = target.map(|path| path.with_extension("partial"));
    let mut file = match (target, &partial) {
        (Some(path), Some(partial)) => {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            Some(std::fs::File::create(partial).map_err(|error| format!("{}: {error}", partial.display())))
        }
        _ => None,
    };
    let mut buffer = vec![0u8; 64 * 1024];
    let mut left = length;
    let mut failure: Option<String> = None;
    while left > 0 {
        let want = left.min(buffer.len() as u64) as usize;
        reader.read_exact(&mut buffer[..want])?;
        left -= want as u64;
        if let (Some(Ok(open)), None) = (&mut file, &failure) {
            if let Err(error) = open.write_all(&buffer[..want]) {
                failure = Some(format!("write failed: {error}"));
            }
        }
    }
    Ok(match (file, failure, target, partial) {
        (Some(Err(error)), _, _, _) => Err(error),
        (_, Some(error), _, _) => Err(error),
        (Some(Ok(open)), None, Some(path), Some(partial)) => {
            drop(open);
            let _ = std::fs::remove_file(path);
            std::fs::rename(&partial, path).map_err(|error| format!("{}: {error}", path.display()))
        }
        _ => Ok(()),
    })
}
