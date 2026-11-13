//! Horizon's own world generator and terrain mesher, as the PS Vita port drives them.
//!
//! Nothing here generates or meshes anything itself: it loads Horizon's prepared 26.3
//! resources, asks `horizon-world` for chunks and `horizon-terrain-mesh` for their
//! quads, and adapts the storage between the two. The adapter follows the Wii U
//! port's bridge (`wiiu/worldgen`). The same code runs in the PC tools and on the
//! console, so what the PC measures is what the console will run.
//!
//! Not wired in yet: Horizon's light engine (every block is fully sky-lit).

use horizon_core::{
    biome::{obfuscate_seed, BiomeZoom},
    position::ChunkPos,
};
use horizon_items::blocks::BlockFacts;
use horizon_terrain_mesh::{
    assets::{Assets, RenderAssets},
    biome::Biomes,
    mesher::{MeshScratch, MeshTables, Mesher},
    models::load_visibility,
    LeafCullingMode, MeshChunk, MeshSectionView, SectionMesh,
};
use horizon_world::worldgen::generator::{GeneratorData, GeneratorWorker, OverworldGenerator, ProtoChunk};
use std::collections::HashMap;
use std::path::Path;

pub use horizon_terrain_data::TerrainVertex;
pub use horizon_terrain_mesh::OPAQUE_LAYER;

const SECTION_VOLUME: usize = 16 * 16 * 16;

/// Blocks and quart biomes of a square of chunks, y/z/x.
pub struct Region {
    blocks: Vec<u32>,
    biomes: Vec<u16>,
    /// Side in blocks.
    diameter: usize,
    pub min_y: i32,
    pub height: usize,
    /// Chunk coordinates of the corner.
    pub origin: (i32, i32),
    /// Side in chunks.
    pub side: usize,
}

impl Region {
    fn state(&self, x: i32, y: i32, z: i32) -> u32 {
        if !(0..self.diameter as i32).contains(&x) || !(0..self.diameter as i32).contains(&z) || !(self.min_y..self.min_y + self.height as i32).contains(&y) {
            return 0;
        }
        self.blocks[((y - self.min_y) as usize * self.diameter + z as usize) * self.diameter + x as usize]
    }

    pub fn sections(&self) -> usize {
        self.height / 16
    }

    pub fn bytes(&self) -> usize {
        self.blocks.len() * 4 + self.biomes.len() * 2
    }
}

struct SectionView<'a> {
    region: &'a Region,
    offset: (usize, usize),
    section_y: i32,
}

impl MeshSectionView for SectionView<'_> {
    fn is_uniform_air(&self) -> bool {
        let mut states = [0u32; SECTION_VOLUME];
        self.get_run(0, &mut states);
        states.iter().all(|&state| state == 0)
    }
    fn unpack(&self, out: &mut [u32; SECTION_VOLUME]) {
        self.get_run(0, out);
    }
    fn get_run(&self, at: usize, out: &mut [u32]) {
        for (offset, block) in out.iter_mut().enumerate() {
            let index = at + offset;
            *block = if index >= SECTION_VOLUME {
                0
            } else {
                self.region.state((self.offset.0 + index % 16) as i32, self.section_y + (index / 256) as i32, (self.offset.1 + index / 16 % 16) as i32)
            };
        }
    }
    fn copy_light_run(&self, _at: usize, out: &mut [u8]) {
        out.fill(0xF0); // full sky light, no block light
    }
}

struct ChunkView<'a> {
    region: &'a Region,
    chunk: (i32, i32),
    offset: (usize, usize),
    sections: Vec<SectionView<'a>>,
}

impl<'a> ChunkView<'a> {
    fn new(region: &'a Region, chunk: (i32, i32)) -> Option<Self> {
        let (dx, dz) = (chunk.0 - region.origin.0, chunk.1 - region.origin.1);
        if dx < 0 || dz < 0 || dx as usize >= region.side || dz as usize >= region.side {
            return None;
        }
        let offset = (dx as usize * 16, dz as usize * 16);
        let sections = (0..region.height / 16).map(|section| SectionView { region, offset, section_y: region.min_y + section as i32 * 16 }).collect();
        Some(ChunkView { region, chunk, offset, sections })
    }
}

impl MeshChunk for ChunkView<'_> {
    fn x(&self) -> i32 {
        self.chunk.0
    }
    fn z(&self) -> i32 {
        self.chunk.1
    }
    fn min_y(&self) -> i32 {
        self.region.min_y
    }
    fn section_at(&self, world_y: i32) -> Result<&dyn MeshSectionView, u8> {
        if world_y < self.region.min_y {
            return Err(0);
        }
        self.sections.get(((world_y - self.region.min_y) / 16) as usize).map(|section| section as &dyn MeshSectionView).ok_or(0xF0)
    }
    fn get(&self, x: i32, y: i32, z: i32) -> u32 {
        self.region.state(self.offset.0 as i32 + (x & 15), y, self.offset.1 as i32 + (z & 15))
    }
    fn light(&self, _x: i32, y: i32, _z: i32) -> u8 {
        if y < self.region.min_y { 0 } else { 0xF0 }
    }
    fn biome(&self, x: i32, y: i32, z: i32) -> u16 {
        let quarts = self.region.diameter / 4;
        let qx = (self.offset.0 / 4 + (x & 15) as usize / 4).min(quarts - 1);
        let qz = (self.offset.1 / 4 + (z & 15) as usize / 4).min(quarts - 1);
        let qy = ((y - self.region.min_y) >> 2).clamp(0, self.region.height as i32 / 4 - 1) as usize;
        self.region.biomes[(qy * quarts + qz) * quarts + qx]
    }
}

/// What decorating a region placed, in Horizon's own counts.
#[derive(Default)]
pub struct Decorated {
    pub chunks: usize,
    pub placed: usize,
    pub unsupported: usize,
    pub failed: usize,
    pub skipped: Vec<String>,
}

/// Horizon's loaded terrain stack: registry, block models and sprites, mesher tables,
/// biome colours and a seeded overworld generator.
pub struct Horizon {
    assets: Assets,
    render: RenderAssets,
    tables: MeshTables,
    biomes: Biomes,
    worker: GeneratorWorker,
    zoom: BiomeZoom,
    pub seed: i64,
    pub spawn: [i32; 2],
}

impl Horizon {
    /// Loads from a prepared 26.3 resource folder. `step` is told each stage's name as
    /// it begins, so a caller can time and weigh the stages.
    pub fn load(root: &Path, seed: i64, mut step: impl FnMut(&str)) -> Result<Self, String> {
        step("registry");
        let assets = Assets::load(root).map_err(|error| format!("assets: {error}"))?;
        let registry = assets.registry;
        step("block models and sprites");
        let render = assets.reload_for_terrain_with_progress(root, |_| {}).map_err(|error| format!("block models and sprites: {error}"))?;
        step("block facts");
        let facts = BlockFacts::load(root, registry).map_err(|error| format!("block facts: {error}"))?;
        step("model visibility");
        let visibility = load_visibility(root, registry).map_err(|error| format!("model visibility: {error}"))?;
        step("mesher tables");
        let tables = MeshTables::new(&assets, &render, &facts, visibility).map_err(|error| format!("mesher tables: {error}"))?;
        drop(facts);
        step("biome colours");
        let biomes = Biomes::load(root).map_err(|error| format!("biome colours: {error}"))?;
        step("worldgen data");
        let data = GeneratorData::load(&root.join("worldgen"), registry).map_err(|error| format!("worldgen data: {error}"))?;
        step("generator");
        // The reduced build has no structure data to place.
        let generator = if cfg!(feature = "terrain-only") { OverworldGenerator::without_structures(data, seed) } else { OverworldGenerator::new(data, seed) }
            .map_err(|error| format!("generator: {error}"))?;
        step("spawn search");
        let spawn = generator.spawn_position().map_err(|error| format!("spawn search: {error}"))?;
        let worker = generator.worker();
        step("ready");
        Ok(Horizon { assets, render, tables, biomes, worker, zoom: BiomeZoom::new(obfuscate_seed(seed)), seed, spawn })
    }

    /// (sprite side in pixels, sprite count, RGBA of every sprite one after another).
    pub fn sprites(&self) -> (usize, usize, std::sync::Arc<Vec<u8>>) {
        (self.render.sprite_size as usize, self.render.texture_layers as usize, self.render.pixels.get())
    }

    /// Generates the chunks within `radius` of `centre`, and one ring more for the
    /// mesher to read at the edges. With `decorate`, also runs Horizon's FEATURES
    /// stage (trees, ores, plants) on them, which needs a second extra ring.
    /// Returns the region and the surface height at the spawn column, when it is inside.
    pub fn generate(&mut self, centre: (i32, i32), radius: i32, decorate: bool) -> Result<(Region, Decorated, Option<i32>), String> {
        let reach = radius + 1 + decorate as i32;
        let mut protos: HashMap<(i32, i32), ProtoChunk> = HashMap::new();
        for cz in centre.1 - reach..=centre.1 + reach {
            for cx in centre.0 - reach..=centre.0 + reach {
                protos.insert((cx, cz), self.worker.generate_proto(ChunkPos::new(cx, cz)).map_err(|error| format!("chunk {cx},{cz}: {error}"))?);
            }
        }
        let mut decorated = Decorated::default();
        if decorate {
            for cz in centre.1 - radius - 1..=centre.1 + radius + 1 {
                for cx in centre.0 - radius - 1..=centre.0 + radius + 1 {
                    let mut around: [ProtoChunk; 9] = std::array::from_fn(|i| protos.remove(&(cx + i as i32 % 3 - 1, cz + i as i32 / 3 - 1)).expect("generated neighbourhood"));
                    let stats = self.worker.decorate(&mut around).map_err(|error| format!("decorating {cx},{cz}: {error}"))?;
                    decorated.chunks += 1;
                    decorated.placed += stats.placed;
                    decorated.unsupported += stats.unsupported;
                    decorated.failed += stats.failed;
                    for name in stats.skipped {
                        if !decorated.skipped.contains(&name) {
                            decorated.skipped.push(name);
                        }
                    }
                    for chunk in around {
                        let position = chunk.position();
                        protos.insert((position.x, position.z), chunk);
                    }
                }
            }
        }
        let spawn_chunk = (self.spawn[0].div_euclid(16), self.spawn[1].div_euclid(16));
        let surface = protos.get(&spawn_chunk).map(|proto| proto.surface_height(self.spawn[0], self.spawn[1]));
        let side = (2 * radius + 3) as usize;
        let region = region_of(&protos, (centre.0 - radius - 1, centre.1 - radius - 1), side)?;
        Ok((region, decorated, surface))
    }

    /// Meshes one chunk of `region` with Horizon's mesher and hands over each quad:
    /// its section index, its four vertices (section-relative positions) and whether
    /// it belongs to the translucent pass.
    pub fn mesh(&self, region: &Region, chunk: (i32, i32), smooth_lighting: bool, mut quad: impl FnMut(usize, &[TerrainVertex], bool)) -> Result<(), String> {
        let views: Vec<Option<ChunkView<'_>>> = (0..9).map(|i| ChunkView::new(region, (chunk.0 + i % 3 - 1, chunk.1 + i / 3 - 1))).collect();
        if views[4].is_none() {
            return Err(format!("chunk {},{} is outside the region", chunk.0, chunk.1));
        }
        let around: [[Option<&ChunkView<'_>>; 3]; 3] = std::array::from_fn(|dz| std::array::from_fn(|dx| views[dz * 3 + dx].as_ref()));
        let mut mesher = Mesher {
            nether_shading: false,
            shader_pack_lighting: false,
            separate_ao: false,
            shader_pack: false,
            assets: &self.assets,
            render: &self.render,
            tables: &self.tables,
            biomes: &self.biomes,
            visibility: self.tables.visibility.clone(),
            zoom: self.zoom,
            blend_radius: 0,
            smooth_lighting,
            cutout_leaves: true,
            translucent_layers: &self.render.translucent_layers,
            leaf_culling: LeafCullingMode::Off,
        };
        let mut scratch = MeshScratch::default();
        let mut mesh = SectionMesh::default();
        for section in 0..region.sections() {
            mesh = mesher.mesh_reusing(&around, section as i32, mesh, &mut scratch);
            for (list, translucent) in [(&mesh.solid, false), (&mesh.translucent, true)] {
                if list.len() % 4 != 0 {
                    return Err("the mesher did not emit quads".to_owned());
                }
                for vertices in list.chunks_exact(4) {
                    quad(section, vertices, translucent);
                }
            }
        }
        Ok(())
    }
}

/// Copies proto-chunks into one region buffer.
fn region_of(protos: &HashMap<(i32, i32), ProtoChunk>, origin: (i32, i32), side: usize) -> Result<Region, String> {
    let any = protos.values().next().ok_or("no chunk")?;
    let (min_y, height) = (any.terrain.min_y(), any.terrain.height() as usize);
    if min_y % 16 != 0 || height % 16 != 0 {
        return Err(format!("unexpected chunk bounds {min_y}+{height}"));
    }
    let diameter = side * 16;
    let quarts = diameter / 4;
    let mut region = Region { blocks: vec![0; height * diameter * diameter], biomes: vec![0; height / 4 * quarts * quarts], diameter, min_y, height, origin, side };
    let mut section = [0u32; SECTION_VOLUME];
    for cz in 0..side {
        for cx in 0..side {
            let proto = protos.get(&(origin.0 + cx as i32, origin.1 + cz as i32)).ok_or("a chunk of the region was not generated")?;
            for sy in 0..height / 16 {
                let section_y = min_y + sy as i32 * 16;
                if !proto.terrain.section_states(section_y, &mut section) {
                    return Err(format!("chunk has no section at y={section_y}"));
                }
                for y in 0..16 {
                    for z in 0..16 {
                        let target = ((sy * 16 + y) * diameter + cz * 16 + z) * diameter + cx * 16;
                        region.blocks[target..target + 16].copy_from_slice(&section[y * 256 + z * 16..][..16]);
                    }
                }
                for qy in 0..4 {
                    for qz in 0..4 {
                        for qx in 0..4 {
                            let id = proto.biome_id_at(qx as i32 * 4, section_y + qy as i32 * 4, qz as i32 * 4).0;
                            region.biomes[((sy * 4 + qy) * quarts + cz * 4 + qz) * quarts + cx * 4 + qx] = u16::try_from(id).map_err(|_| format!("biome id {id} does not fit the mesher's storage"))?;
                        }
                    }
                }
            }
        }
    }
    Ok(region)
}
