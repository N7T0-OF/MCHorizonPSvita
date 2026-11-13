//! Times and weighs each stage of Horizon's terrain start-up on the PC, then a small
//! region, through the same `vita/world` code the console runs.
//!
//! Build it for 32 bits to see what a PS Vita would hold in memory:
//!   cargo run --release --target i686-pc-windows-msvc --features terrain-only --bin phases -- <resources>
//!
//! usage: phases <prepared-26.3-resources> [seed]

use horizon_vita_world::Horizon;
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;

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

#[global_allocator]
static HEAP: Counting = Counting;

fn mib(bytes: usize) -> f64 {
    bytes as f64 / 1048576.0
}

fn main() {
    let mut args = std::env::args().skip(1);
    let root = args.next().expect("usage: <prepared-26.3-resources> [seed]");
    let seed: i64 = args.next().map_or(20261112, |s| s.parse().expect("seed is an integer"));

    // Each stage is reported when the next one begins.
    let mut stage: Option<(String, Instant, usize)> = None;
    let mut report = |next: &str| {
        if let Some((name, began, before)) = stage.take() {
            let live = LIVE.load(Relaxed);
            println!("{name:<28} {:>7.2} s   live {:>7.1} MiB ({:+.1})   peak so far {:>7.1} MiB", began.elapsed().as_secs_f32(), mib(live), mib(live) - mib(before), mib(PEAK.load(Relaxed)));
        }
        stage = Some((next.to_owned(), Instant::now(), LIVE.load(Relaxed)));
    };
    let mut horizon = Horizon::load(Path::new(&root), seed, &mut report).expect("Horizon");
    let (sprite, layers, pixels) = horizon.sprites();
    println!("sprites: {layers} of {sprite} px = {:.1} MiB RGBA; spawn {:?}", mib(pixels.len()), horizon.spawn);
    drop(pixels);

    let centre = (horizon.spawn[0].div_euclid(16), horizon.spawn[1].div_euclid(16));
    for (radius, decorate) in [(0, false), (1, false), (1, !cfg!(feature = "terrain-only"))] {
        let began = Instant::now();
        let (region, decorated, _) = horizon.generate(centre, radius, decorate).expect("generation");
        let generated = began.elapsed();
        let began = Instant::now();
        let mut quads = 0usize;
        for cz in centre.1 - radius..=centre.1 + radius {
            for cx in centre.0 - radius..=centre.0 + radius {
                horizon.mesh(&region, (cx, cz), false, |_, _, _| quads += 1).expect("meshing");
            }
        }
        let side = 2 * radius + 1;
        println!(
            "{side}x{side} chunks{}: generate {:.3} s, mesh {:.3} s, {quads} quads, region {:.1} MiB, live {:.1} MiB, peak {:.1} MiB",
            if decorate { format!(" decorated ({} features)", decorated.placed) } else { String::new() },
            generated.as_secs_f32(),
            began.elapsed().as_secs_f32(),
            mib(region.bytes()),
            mib(LIVE.load(Relaxed)),
            mib(PEAK.load(Relaxed))
        );
    }
}
