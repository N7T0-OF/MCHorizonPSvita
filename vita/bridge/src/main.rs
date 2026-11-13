//! PC client of the bridge in `vita/engine/src/bridge.rs`.
//!
//! usage: vita-bridge <console-ip[:port]> <command>
//!   ping | state | reload | quit
//!   key <up|down|left|right|cross|circle|square|triangle|start|select|l|r>
//!   cam <x> <y> <z> <yaw> <pitch>
//!   set <vsync|cull|faces|gpu> <integer>
//!   shot <out.png>            the picture on the console's screen
//!   log                       ux0:data/HorizonVita/horizon.log
//!   get <remote> <local>      a file under ux0:data/HorizonVita
//!   put <local> <remote>      send a file there
//!
//! It only ever connects out to the console: nothing listens on the PC.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const WIDTH: u32 = 960;
const HEIGHT: u32 = 544;

fn request(stream: &TcpStream, line: &str, body: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let io = |error: std::io::Error| format!("connection: {error}");
    let mut out = stream;
    out.write_all(format!("{line}\n").as_bytes()).map_err(io)?;
    if let Some(body) = body {
        out.write_all(body).map_err(io)?;
    }
    out.flush().map_err(io)?;
    let mut reader = BufReader::new(stream);
    let mut head = String::new();
    reader.read_line(&mut head).map_err(io)?;
    let head = head.trim_end();
    if let Some(message) = head.strip_prefix("ERR ") {
        return Err(format!("the console refused: {message}"));
    }
    let length: usize = head.strip_prefix("OK ").and_then(|n| n.parse().ok()).ok_or_else(|| format!("unexpected answer {head:?}"))?;
    let mut bytes = vec![0u8; length];
    reader.read_exact(&mut bytes).map_err(io)?;
    Ok(bytes)
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: vita-bridge <console-ip[:port]> <command> [arguments]  (see the top of vita/bridge/src/main.rs)";
    let (host, command) = match args.as_slice() {
        [host, command, ..] => (host.as_str(), command.as_str()),
        _ => return Err(usage.to_owned()),
    };
    let rest = &args[2..];
    let address = if host.contains(':') { host.to_owned() } else { format!("{host}:4747") };
    let address = address.to_socket_addrs().map_err(|error| format!("{address}: {error}"))?.next().ok_or("no address")?;
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5)).map_err(|error| format!("{address}: {error} (is Horizon Vita open, with PONT PC on?)"))?;
    stream.set_read_timeout(Some(Duration::from_secs(60))).ok();
    stream.set_nodelay(true).ok();
    let text = |bytes: Vec<u8>| String::from_utf8_lossy(&bytes).into_owned();
    match (command, rest) {
        ("ping" | "state" | "reload" | "quit", []) => println!("{}", text(request(&stream, command, None)?)),
        ("key" | "cam" | "set", _) if !rest.is_empty() => println!("{}", text(request(&stream, &format!("{command} {}", rest.join(" ")), None)?)),
        ("log", []) => print!("{}", text(request(&stream, "get horizon.log", None)?)),
        ("get", [remote, local]) => {
            let bytes = request(&stream, &format!("get {remote}"), None)?;
            std::fs::write(local, &bytes).map_err(|error| format!("{local}: {error}"))?;
            println!("{local}: {} bytes", bytes.len());
        }
        ("put", [local, remote]) => {
            let bytes = std::fs::read(local).map_err(|error| format!("{local}: {error}"))?;
            println!("{}", text(request(&stream, &format!("put {remote} {}", bytes.len()), Some(&bytes))?));
        }
        ("shot", [out]) => {
            let pixels = request(&stream, "shot", None)?;
            if pixels.len() != (WIDTH * HEIGHT * 4) as usize {
                return Err(format!("unexpected picture size {}", pixels.len()));
            }
            let file = std::fs::File::create(out).map_err(|error| format!("{out}: {error}"))?;
            let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let rgb: Vec<u8> = pixels.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
            encoder.write_header().and_then(|mut writer| writer.write_image_data(&rgb)).map_err(|error| format!("{out}: {error}"))?;
            println!("{out}: {WIDTH}x{HEIGHT}");
        }
        _ => return Err(usage.to_owned()),
    }
    Ok(())
}

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
