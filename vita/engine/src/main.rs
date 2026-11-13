//! Horizon Vita: the single PS Vita application of the port.
//!
//! One binary holds the menu, the diagnostics, the mod folders and the 3D renderer,
//! because a Vita homebrew cannot simply start another program. It is NOT the game
//! yet: PLAY opens the hardware 3D test (perspective, depth, chunked meshes, stick
//! camera), whose terrain is a stand-in built from `horizon-core`'s noise.
//!
//! On the PC (`cargo run --release`) it prints the checksums the console must show.

mod bridge;
mod horizon_test;
mod terrain;

#[cfg(target_os = "vita")]
mod app;
#[cfg(target_os = "vita")]
mod gfx;

use terrain::{World, CHUNKS_PER_SIDE};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const FIRST_SEED: i64 = 20261112;
/// What `terrain::noise_checksum(FIRST_SEED)` and `World::new(FIRST_SEED).checksum()` give on
/// the PC. The console must compute the same values from the same `horizon-core` code.
pub const EXPECTED_NOISE: u32 = 0xF748_DA62;
pub const EXPECTED_WORLD: u32 = 0xDF77_845F;

/// The size of the C library's heap, which is also the Rust heap: it is reserved once
/// at start-up (128 MiB when this is absent). The application has about 365 MiB in
/// all (measured: 230 MiB free with the default heap). Horizon's terrain stack peaks
/// near 160 MiB while it loads.
#[cfg(target_os = "vita")]
#[used]
#[no_mangle]
pub static _newlib_heap_size_user: u32 = 224 << 20;

/// Counts what the Rust heap holds.
pub mod heap {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    pub static LIVE: AtomicUsize = AtomicUsize::new(0);
    pub static PEAK: AtomicUsize = AtomicUsize::new(0);

    pub struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                let live = LIVE.fetch_add(layout.size(), Relaxed) + layout.size();
                PEAK.fetch_max(live, Relaxed);
            }
            ptr
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) };
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
    }
}

#[global_allocator]
static HEAP: heap::Counting = heap::Counting;

pub struct Built {
    pub world: World,
    pub meshes: Vec<terrain::ChunkMesh>,
    pub generate: std::time::Duration,
    pub mesh: std::time::Duration,
}

pub fn build(seed: i64) -> Built {
    let start = std::time::Instant::now();
    let world = World::new(seed);
    let generate = start.elapsed();
    let start = std::time::Instant::now();
    let mut meshes = Vec::with_capacity(CHUNKS_PER_SIDE * CHUNKS_PER_SIDE);
    for cz in 0..CHUNKS_PER_SIDE {
        for cx in 0..CHUNKS_PER_SIDE {
            meshes.push(world.mesh_chunk(cx, cz));
        }
    }
    Built { world, meshes, generate, mesh: start.elapsed() }
}

#[cfg(not(target_os = "vita"))]
fn main() {
    // `bridge <folder>`: the console's bridge server on the PC, with made-up answers
    // from the frame loop, to test the protocol and `vita-bridge` without a console.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [mode, root] = args.as_slice() {
        if mode == "horizon" {
            // The console's Horizon test, on the PC: the reference for its numbers.
            let report = horizon_test::start(root.into(), FIRST_SEED, || 0).expect("test thread");
            let mut shown = 0;
            loop {
                let lines = report.lock().unwrap().clone();
                for line in &lines[shown..] {
                    println!("{line}");
                }
                shown = lines.len();
                if lines.last().is_some_and(|last| last == "DONE" || last.starts_with("FAILED")) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        if mode == "bridge" {
            let server = bridge::Bridge::start(root.clone(), VERSION);
            println!("bridge test server on port {}, files under {root}", bridge::PORT);
            loop {
                while let Some(envelope) = server.poll() {
                    let answer = match envelope.request {
                        bridge::Request::State => Ok(b"version test\nscreen menu\n".to_vec()),
                        bridge::Request::Shot => Ok((0..960 * 544).flat_map(|i| [(i % 960 / 4) as u8, (i / 960 / 3) as u8, 96, 255]).collect()),
                        bridge::Request::Key(name) => Ok(format!("key {name}").into_bytes()),
                        bridge::Request::Camera(camera) => Ok(format!("camera {camera:?}").into_bytes()),
                        bridge::Request::Set(name, value) => Ok(format!("{name} = {value}").into_bytes()),
                        bridge::Request::Reload => Ok(b"reloaded".to_vec()),
                        bridge::Request::Run(what) => Ok(format!("run {what}").into_bytes()),
                        bridge::Request::Quit => std::process::exit(0),
                    };
                    let _ = envelope.reply.send(answer);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
    let built = build(FIRST_SEED);
    let vertices: usize = built.meshes.iter().map(|m| m.vertices.len()).sum();
    let indices: usize = built.meshes.iter().map(|m| m.indices.len()).sum();
    let (noise, world) = (terrain::noise_checksum(FIRST_SEED), built.world.checksum());
    println!(
        "seed {FIRST_SEED}: noise {noise:08X}, world {world:08X}, {} chunks, {vertices} vertices, {} triangles, generate {:?}, mesh {:?}, heap peak {} bytes",
        built.meshes.len(),
        indices / 3,
        built.generate,
        built.mesh,
        heap::PEAK.load(std::sync::atomic::Ordering::Relaxed)
    );
    assert_eq!(noise, EXPECTED_NOISE, "the reference in main.rs is stale");
    assert_eq!(world, EXPECTED_WORLD, "the reference in main.rs is stale");
}

#[cfg(target_os = "vita")]
fn main() {
    app::run();
}
