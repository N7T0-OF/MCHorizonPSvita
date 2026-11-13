//! Runs Horizon's real terrain stack where this program runs, on a thread of its own,
//! and reports what each stage costs: the registry, block models and sprites, mesher
//! tables and the 26.3 overworld generator (the reduced build the Wii U port uses),
//! then a few chunks generated and meshed by Horizon's own code.
//!
//! On the console this is the first time Horizon's world code itself runs there. It
//! draws nothing yet: it answers whether it loads, how long it takes and what it holds.

use horizon_vita_world::Horizon;
use std::path::PathBuf;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Lines the test wrote so far; the last one says how it ended.
pub type Report = Arc<Mutex<Vec<String>>>;

/// Starts the test. `free_kib` reads the system's free memory, where there is such a thing.
pub fn start(root: PathBuf, seed: i64, free_kib: fn() -> i32) -> Result<Report, String> {
    let report: Report = Arc::new(Mutex::new(vec![format!("Horizon test: resources {}", root.display())]));
    let lines = report.clone();
    // Horizon's loaders and generator recurse and keep large scratch arrays: give
    // them a desktop-sized stack rather than the console's default.
    std::thread::Builder::new()
        .name("horizon-test".into())
        .stack_size(8 << 20)
        .spawn(move || {
            let say = |line: String| lines.lock().unwrap().push(line);
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&root, seed, free_kib, &say)));
            say(match outcome {
                Ok(Ok(())) => "DONE".to_owned(),
                Ok(Err(error)) => format!("FAILED: {error}"),
                Err(_) => "FAILED: panic (see horizon-panic.txt)".to_owned(),
            });
        })
        .map_err(|error| format!("thread: {error}"))?;
    Ok(report)
}

fn run(root: &PathBuf, seed: i64, free_kib: fn() -> i32, say: &dyn Fn(String)) -> Result<(), String> {
    let heap = || (crate::heap::LIVE.load(Relaxed) / 1024, crate::heap::PEAK.load(Relaxed) / 1024);
    let began = Instant::now();
    // Each stage is reported when the next one begins.
    let mut stage: Option<(String, Instant)> = None;
    let mut horizon = Horizon::load(root, seed, |next| {
        if let Some((name, started)) = stage.take() {
            let (live, peak) = heap();
            say(format!("{name}: {:.2} s, heap {live} KiB, peak {peak} KiB, free {} KiB", started.elapsed().as_secs_f32(), free_kib()));
        }
        stage = Some((next.to_owned(), Instant::now()));
    })?;
    let (sprite, layers, pixels) = horizon.sprites();
    say(format!("loaded in {:.1} s: {layers} sprites of {sprite} px ({} KiB), spawn {:?}", began.elapsed().as_secs_f32(), pixels.len() / 1024, horizon.spawn));
    drop(pixels);

    let centre = (horizon.spawn[0].div_euclid(16), horizon.spawn[1].div_euclid(16));
    for radius in [0, 1] {
        let started = Instant::now();
        let (region, _, surface) = horizon.generate(centre, radius, false)?;
        let generated = started.elapsed();
        let started = Instant::now();
        let (mut quads, mut checksum) = (0usize, 0x811c_9dc5u32);
        for cz in centre.1 - radius..=centre.1 + radius {
            for cx in centre.0 - radius..=centre.0 + radius {
                horizon.mesh(&region, (cx, cz), false, |_, quad, _| {
                    quads += 1;
                    // FNV-1a over what the mesher produced, to compare console and PC.
                    for vertex in quad {
                        for byte in vertex.pos.iter().flat_map(|v| v.to_bits().to_le_bytes()).chain(vertex.layer.to_le_bytes()).chain(vertex.color.to_le_bytes()) {
                            checksum = (checksum ^ byte as u32).wrapping_mul(0x0100_0193);
                        }
                    }
                })?;
            }
        }
        let side = 2 * radius + 1;
        let (live, peak) = heap();
        say(format!(
            "{side}x{side} chunks: generate {:.2} s, mesh {:.2} s, {quads} quads, mesh checksum {checksum:08X}, surface {surface:?}, heap {live} KiB, peak {peak} KiB, free {} KiB",
            generated.as_secs_f32(),
            started.elapsed().as_secs_f32(),
            free_kib()
        ));
    }
    Ok(())
}
