//! Times and weighs each step of Horizon's terrain start-up on the PC (the steps of
//! `wiiu/worldgen`'s `load_world`), to find what a PS Vita cannot afford.
//!
//! usage: phases <prepared-26.3-resources> [seed]

use horizon_items::blocks::BlockFacts;
use horizon_terrain_mesh::{assets::Assets, biome::Biomes, mesher::MeshTables, models::load_visibility};
use horizon_world::worldgen::generator::{GeneratorData, OverworldGenerator};
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

struct Meter(Instant, usize);

impl Meter {
    fn step(&mut self, name: &str) {
        let live = LIVE.load(Relaxed);
        println!(
            "{name:<34} {:>7.2} s   live {:>7.1} MiB ({:+.1})   peak so far {:>7.1} MiB",
            self.0.elapsed().as_secs_f32(),
            live as f64 / 1048576.0,
            (live as f64 - self.1 as f64) / 1048576.0,
            PEAK.load(Relaxed) as f64 / 1048576.0
        );
        (self.0, self.1) = (Instant::now(), live);
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let root = args.next().expect("usage: <prepared-26.3-resources> [seed]");
    let root = Path::new(&root);
    let seed: i64 = args.next().map_or(20261112, |s| s.parse().expect("seed is an integer"));
    let mut meter = Meter(Instant::now(), 0);

    let assets = Assets::load(root).expect("assets");
    let registry = assets.registry;
    meter.step("Assets::load (registry)");
    let render = assets.reload_for_terrain_with_progress(root, |_| {}).expect("block models and sprites");
    println!("  sprites: {} layers of {} px = {:.1} MiB RGBA", render.texture_layers, render.sprite_size, render.pixels.get().len() as f64 / 1048576.0);
    meter.step("reload_for_terrain (models, sprites)");
    let facts = BlockFacts::load(root, registry).expect("block facts");
    meter.step("BlockFacts::load");
    let visibility = load_visibility(root, registry).expect("visibility");
    meter.step("load_visibility");
    let tables = MeshTables::new(&assets, &render, &facts, visibility).expect("mesh tables");
    meter.step("MeshTables::new");
    let biomes = Biomes::load(root).expect("biome colours");
    meter.step("Biomes::load");
    let data = GeneratorData::load(&root.join("worldgen"), registry).expect("worldgen data");
    meter.step("GeneratorData::load");
    let generator = OverworldGenerator::without_structures(data, seed).expect("generator");
    meter.step("OverworldGenerator::new");
    let spawn = generator.spawn_position().expect("spawn");
    println!("  spawn {spawn:?}");
    meter.step("spawn_position");
    drop(facts);
    meter.step("drop BlockFacts");
    let _keep = (tables, biomes, generator, render, registry.len());
}
