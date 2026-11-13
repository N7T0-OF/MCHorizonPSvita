//! Packs a region of a real Horizon world for the PS Vita to draw.
//!
//! Everything about the world is Horizon's own code running on the PC: the 26.3
//! overworld generator (terrain, carvers, structures, then the FEATURES decoration:
//! trees, ores, plants) and the canonical terrain mesher with the prepared block
//! models and sprites. The console only draws the result.
//!
//! What this file adds is the reduction to what the console can draw today, with
//! libvita2d's precompiled texture shader (a texture, no vertex colours):
//!   - each quad's tint and face shading is baked into its own atlas cell, so the
//!     colours are per quad (no smooth lighting);
//!   - sprites are reduced to 16 px;
//!   - every block is fully sky-lit, as in the Wii U bridge this is adapted from
//!     (`wiiu/worldgen`): Horizon's light engine is not wired in.
//!
//! The pack holds textures and terrain derived from the user's own copy of Minecraft:
//! it stays on their PC and console and must never be committed or shared.
//!
//! usage: pack <prepared-26.3-resources> <out.hvw> [seed] [radius]

use horizon_core::{
    biome::{obfuscate_seed, BiomeZoom},
    position::ChunkPos,
};
use horizon_items::blocks::BlockFacts;
use horizon_terrain_data::TerrainVertex;
use horizon_terrain_mesh::{
    assets::Assets,
    biome::Biomes,
    mesher::{MeshScratch, MeshTables, Mesher},
    models::load_visibility,
    LeafCullingMode, MeshChunk, MeshSectionView, SectionMesh, OPAQUE_LAYER,
};
use horizon_world::worldgen::generator::{GeneratorData, OverworldGenerator, ProtoChunk};
use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;
use std::time::Instant;

const SECTION_VOLUME: usize = 16 * 16 * 16;
/// Pixels of a sprite in the console's atlas, and of its cell with a 1 px border.
const TEXELS: usize = 16;
const CELL: usize = TEXELS + 2;
/// The console draws quads with a shared 16-bit index pattern: 4 vertices each.
const MAX_BATCH_QUADS: usize = 16384;

const OPAQUE: u8 = 0;
const CUTOUT: u8 = 1;
const TRANSLUCENT: u8 = 2;

/// Blocks and quart biomes of a square of chunks, y/z/x.
struct Region {
    blocks: Vec<u32>,
    biomes: Vec<u16>,
    /// Side in blocks.
    diameter: usize,
    min_y: i32,
    height: usize,
    /// Chunk coordinates of the corner.
    origin: (i32, i32),
}

impl Region {
    fn state(&self, x: i32, y: i32, z: i32) -> u32 {
        if !(0..self.diameter as i32).contains(&x) || !(0..self.diameter as i32).contains(&z) || !(self.min_y..self.min_y + self.height as i32).contains(&y) {
            return 0;
        }
        self.blocks[((y - self.min_y) as usize * self.diameter + z as usize) * self.diameter + x as usize]
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
    fn new(region: &'a Region, chunk: (i32, i32)) -> Self {
        let offset = ((chunk.0 - region.origin.0) as usize * 16, (chunk.1 - region.origin.1) as usize * 16);
        let sections = (0..region.height / 16).map(|section| SectionView { region, offset, section_y: region.min_y + section as i32 * 16 }).collect();
        ChunkView { region, chunk, offset, sections }
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

/// Copies decorated proto-chunks into one region buffer.
fn region_of(protos: &HashMap<(i32, i32), ProtoChunk>, origin: (i32, i32), side: usize) -> Region {
    let any = protos.values().next().expect("at least one chunk");
    let (min_y, height) = (any.terrain.min_y(), any.terrain.height() as usize);
    assert!(min_y % 16 == 0 && height % 16 == 0, "unexpected chunk bounds {min_y}+{height}");
    let diameter = side * 16;
    let quarts = diameter / 4;
    let mut region = Region { blocks: vec![0; height * diameter * diameter], biomes: vec![0; height / 4 * quarts * quarts], diameter, min_y, height, origin };
    let mut section = [0u32; SECTION_VOLUME];
    for cz in 0..side {
        for cx in 0..side {
            let proto = &protos[&(origin.0 + cx as i32, origin.1 + cz as i32)];
            for sy in 0..height / 16 {
                let section_y = min_y + sy as i32 * 16;
                assert!(proto.terrain.section_states(section_y, &mut section), "chunk has no section at y={section_y}");
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
                            region.biomes[((sy * 4 + qy) * quarts + cz * 4 + qz) * quarts + cx * 4 + qx] = u16::try_from(id).expect("biome id fits the mesher's storage");
                        }
                    }
                }
            }
        }
    }
    region
}

/// What makes two quads share an atlas cell.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct CellKey {
    layer: u16,
    /// The quad's colour, 5 bits per channel.
    colour: [u8; 3],
    mode: u8,
}

struct Quad {
    positions: [[f32; 3]; 4],
    uvs: [[f32; 2]; 4],
    cell: u32,
}

#[derive(Default)]
struct Batch {
    mode: u8,
    quads: Vec<Quad>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: pack <prepared-26.3-resources> <out.hvw> [seed] [radius]";
    let root = args.next().expect(usage);
    let root = Path::new(&root);
    let out = args.next().expect(usage);
    let seed: i64 = args.next().map_or(20261112, |s| s.parse().expect("seed is an integer"));
    let radius: i32 = args.next().map_or(2, |s| s.parse().expect("radius is an integer"));
    assert!((0..=6).contains(&radius), "radius 0 to 6");

    let start = Instant::now();
    let assets = Assets::load(root).expect("assets");
    let registry = assets.registry;
    let render = assets.reload_for_terrain_with_progress(root, |_| {}).expect("block models and sprites");
    let facts = BlockFacts::load(root, registry).expect("block facts");
    let visibility = load_visibility(root, registry).expect("visibility");
    let tables = MeshTables::new(&assets, &render, &facts, visibility).expect("mesh tables");
    let biomes = Biomes::load(root).expect("biome colours");
    let data = GeneratorData::load(&root.join("worldgen"), registry).expect("worldgen data");
    let generator = OverworldGenerator::new(data, seed).expect("generator");
    let spawn = generator.spawn_position().expect("spawn");
    let mut worker = generator.worker();
    println!("Horizon loaded in {:.1} s; seed {seed}, spawn {spawn:?}", start.elapsed().as_secs_f32());

    // Decorating a chunk writes into its neighbours, and meshing a chunk reads its
    // neighbours: generate two rings more than is packed, decorate one ring more.
    let start = Instant::now();
    let centre = (spawn[0].div_euclid(16), spawn[1].div_euclid(16));
    let mut protos = HashMap::new();
    for cz in centre.1 - radius - 2..=centre.1 + radius + 2 {
        for cx in centre.0 - radius - 2..=centre.0 + radius + 2 {
            protos.insert((cx, cz), worker.generate_proto(ChunkPos::new(cx, cz)).expect("chunk generation"));
        }
    }
    let (mut placed, mut unsupported, mut failed) = (0, 0, 0);
    let mut skipped = std::collections::BTreeSet::new();
    for cz in centre.1 - radius - 1..=centre.1 + radius + 1 {
        for cx in centre.0 - radius - 1..=centre.0 + radius + 1 {
            let mut around: [ProtoChunk; 9] = std::array::from_fn(|i| protos.remove(&(cx + i as i32 % 3 - 1, cz + i as i32 / 3 - 1)).expect("generated neighbourhood"));
            let stats = worker.decorate(&mut around).expect("decoration");
            placed += stats.placed;
            unsupported += stats.unsupported;
            failed += stats.failed;
            skipped.extend(stats.skipped);
            for chunk in around {
                let position = chunk.position();
                protos.insert((position.x, position.z), chunk);
            }
        }
    }
    println!(
        "generated {} chunks and decorated {} in {:.1} s: {placed} features placed, {unsupported} unsupported, {failed} failed",
        protos.len(),
        (2 * radius + 3) * (2 * radius + 3),
        start.elapsed().as_secs_f32()
    );
    if !skipped.is_empty() {
        println!("  skipped features: {}", skipped.iter().cloned().collect::<Vec<_>>().join(", "));
    }
    let surface = protos[&centre].surface_height(spawn[0], spawn[1]);

    let side = (2 * radius + 3) as usize;
    let region = region_of(&protos, (centre.0 - radius - 1, centre.1 - radius - 1), side);
    drop(protos);

    // Sprite facts for the atlas.
    let sprite = render.sprite_size as usize;
    let layers = render.texture_layers as usize;
    let pixels = render.pixels.get();
    assert_eq!(pixels.len(), layers * sprite * sprite * 4, "sprite pixels");
    let has_alpha: Vec<bool> = (0..layers).map(|layer| pixels[layer * sprite * sprite * 4..][..sprite * sprite * 4].chunks_exact(4).any(|p| p[3] < 255)).collect();

    let start = Instant::now();
    let mut mesher = Mesher {
        nether_shading: false,
        shader_pack_lighting: false,
        separate_ao: false,
        shader_pack: false,
        assets: &assets,
        render: &render,
        tables: &tables,
        biomes: &biomes,
        visibility: tables.visibility.clone(),
        zoom: BiomeZoom::new(obfuscate_seed(seed)),
        blend_radius: 0,
        // Flat lighting: one colour per quad, which is what an atlas cell can hold.
        smooth_lighting: false,
        cutout_leaves: true,
        translucent_layers: &render.translucent_layers,
        leaf_culling: LeafCullingMode::Off,
    };
    let views: HashMap<(i32, i32), ChunkView<'_>> =
        (0..side * side).map(|i| (region.origin.0 + (i % side) as i32, region.origin.1 + (i / side) as i32)).map(|chunk| (chunk, ChunkView::new(&region, chunk))).collect();
    let mut scratch = MeshScratch::default();
    let mut mesh = SectionMesh::default();
    let mut cells: HashMap<CellKey, u32> = HashMap::new();
    let mut cell_keys: Vec<CellKey> = Vec::new();
    let mut batches: Vec<Batch> = Vec::new();
    let mut counts = [0usize; 3];
    // Packed coordinates start at the packed area's corner, so they stay small.
    let packed_origin = (centre.0 - radius, centre.1 - radius);
    for cz in centre.1 - radius..=centre.1 + radius {
        for cx in centre.0 - radius..=centre.0 + radius {
            let around: [[Option<&ChunkView<'_>>; 3]; 3] = std::array::from_fn(|dz| std::array::from_fn(|dx| views.get(&(cx + dx as i32 - 1, cz + dz as i32 - 1))));
            for section in 0..region.height / 16 {
                mesh = mesher.mesh_reusing(&around, section as i32, mesh, &mut scratch);
                let base = [((cx - packed_origin.0) * 16) as f32, (region.min_y + section as i32 * 16) as f32, ((cz - packed_origin.1) * 16) as f32];
                let mut section_batches: [Batch; 3] = std::array::from_fn(|mode| Batch { mode: mode as u8, quads: Vec::new() });
                for (list, translucent) in [(&mesh.solid, false), (&mesh.translucent, true)] {
                    assert!(list.len() % 4 == 0, "the mesher emits quads");
                    for quad in list.chunks_exact(4) {
                        let layer = quad[0].layer & !OPAQUE_LAYER;
                        assert!((layer as usize) < layers, "sprite layer {layer} of {layers}");
                        let mode = if translucent {
                            TRANSLUCENT
                        } else if quad[0].layer & OPAQUE_LAYER != 0 || !has_alpha[layer as usize] {
                            OPAQUE
                        } else {
                            CUTOUT
                        };
                        let mut colour = [0u32; 3];
                        for vertex in quad {
                            for (sum, channel) in colour.iter_mut().zip(vertex.rgba()) {
                                *sum += channel as u32;
                            }
                        }
                        let key = CellKey { layer, colour: colour.map(|sum| ((sum / 4 + 4) >> 3).min(31) as u8), mode };
                        let cell = *cells.entry(key).or_insert_with(|| {
                            cell_keys.push(key);
                            cell_keys.len() as u32 - 1
                        });
                        counts[mode as usize] += 1;
                        section_batches[mode as usize].quads.push(Quad {
                            positions: std::array::from_fn(|i| std::array::from_fn(|axis| quad[i].pos[axis] + base[axis])),
                            uvs: std::array::from_fn(|i| TerrainVertex::uv(&quad[i]).map(|v| v.clamp(0.0, 1.0))),
                            cell,
                        });
                    }
                }
                for batch in section_batches {
                    let mut quads = batch.quads;
                    while !quads.is_empty() {
                        let rest = quads.split_off(quads.len().min(MAX_BATCH_QUADS));
                        batches.push(Batch { mode: batch.mode, quads });
                        quads = rest;
                    }
                }
            }
        }
    }
    batches.sort_by_key(|batch| batch.mode); // stable: opaque, then cutout, then translucent
    let total: usize = counts.iter().sum();
    println!(
        "meshed {} chunks in {:.1} s: {total} quads ({} opaque, {} cutout, {} translucent) in {} batches, {} atlas cells",
        (2 * radius + 1) * (2 * radius + 1),
        start.elapsed().as_secs_f32(),
        counts[0],
        counts[1],
        counts[2],
        batches.len(),
        cell_keys.len()
    );

    // The atlas: one cell per (sprite, colour, mode).
    let width = [1024usize, 2048, 4096].into_iter().find(|w| (w / CELL) * (w / CELL) >= cell_keys.len()).expect("too many atlas cells for a 4096 px texture");
    let columns = width / CELL;
    let height = (cell_keys.len().div_ceil(columns) * CELL).next_power_of_two().max(64);
    let mut atlas = vec![0u8; width * height * 4];
    for (index, key) in cell_keys.iter().enumerate() {
        let source = &pixels[key.layer as usize * sprite * sprite * 4..][..sprite * sprite * 4];
        let tint = key.colour.map(|c| c as u32 * 255 / 31);
        // Reduce the sprite to TEXELS x TEXELS (alpha-weighted box filter), tinted.
        let mut texels = [[0u8; 4]; TEXELS * TEXELS];
        for ty in 0..TEXELS {
            for tx in 0..TEXELS {
                let (x0, x1) = (tx * sprite / TEXELS, ((tx + 1) * sprite / TEXELS).max(tx * sprite / TEXELS + 1));
                let (y0, y1) = (ty * sprite / TEXELS, ((ty + 1) * sprite / TEXELS).max(ty * sprite / TEXELS + 1));
                let (mut rgb, mut alpha, mut plain, mut samples) = ([0u32; 3], 0u32, [0u32; 3], 0u32);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let p = &source[(y.min(sprite - 1) * sprite + x.min(sprite - 1)) * 4..][..4];
                        for c in 0..3 {
                            rgb[c] += p[c] as u32 * p[3] as u32;
                            plain[c] += p[c] as u32;
                        }
                        alpha += p[3] as u32;
                        samples += 1;
                    }
                }
                let colour: [u32; 3] = std::array::from_fn(|c| if alpha > 0 { rgb[c] / alpha } else { plain[c] / samples });
                let a = match key.mode {
                    OPAQUE => 255,
                    CUTOUT => if alpha / samples >= 128 { 255 } else { 0 },
                    _ => alpha / samples,
                };
                texels[ty * TEXELS + tx] = [(colour[0] * tint[0] / 255) as u8, (colour[1] * tint[1] / 255) as u8, (colour[2] * tint[2] / 255) as u8, a as u8];
            }
        }
        let (cell_x, cell_y) = (index % columns * CELL, index / columns * CELL);
        for y in 0..CELL {
            for x in 0..CELL {
                let texel = texels[y.saturating_sub(1).min(TEXELS - 1) * TEXELS + x.saturating_sub(1).min(TEXELS - 1)];
                atlas[((cell_y + y) * width + cell_x + x) * 4..][..4].copy_from_slice(&texel);
            }
        }
    }

    // The file. All little-endian; see vita/engine/src/pack.rs for the reader.
    let mut file = std::io::BufWriter::new(std::fs::File::create(&out).expect("output file"));
    let camera = [(spawn[0] - packed_origin.0 * 16) as f32 + 0.5, surface as f32 + 2.6, (spawn[1] - packed_origin.1 * 16) as f32 + 0.5];
    let mut header = Vec::with_capacity(64);
    header.extend_from_slice(b"HVW1");
    for value in [1u32, width as u32, height as u32, batches.len() as u32, total as u32] {
        header.extend_from_slice(&value.to_le_bytes());
    }
    for value in camera {
        header.extend_from_slice(&value.to_le_bytes());
    }
    header.extend_from_slice(&seed.to_le_bytes());
    for value in [packed_origin.0, packed_origin.1, radius] {
        header.extend_from_slice(&value.to_le_bytes());
    }
    header.resize(64, 0);
    file.write_all(&header).unwrap();
    file.write_all(&atlas).unwrap();
    let mut first = 0u32;
    for batch in &batches {
        let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
        for position in batch.quads.iter().flat_map(|quad| quad.positions) {
            for axis in 0..3 {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
        }
        for value in min.into_iter().chain(max) {
            file.write_all(&value.to_le_bytes()).unwrap();
        }
        for value in [batch.mode as u32, first, batch.quads.len() as u32] {
            file.write_all(&value.to_le_bytes()).unwrap();
        }
        first += batch.quads.len() as u32;
    }
    for quad in batches.iter().flat_map(|batch| &batch.quads) {
        let (cell_x, cell_y) = (quad.cell as usize % columns * CELL, quad.cell as usize / columns * CELL);
        for (position, uv) in quad.positions.iter().zip(quad.uvs) {
            let atlas_uv = [(cell_x as f32 + 1.0 + uv[0] * TEXELS as f32) / width as f32, (cell_y as f32 + 1.0 + uv[1] * TEXELS as f32) / height as f32];
            for value in position.iter().chain(&atlas_uv) {
                file.write_all(&value.to_le_bytes()).unwrap();
            }
        }
    }
    file.flush().unwrap();
    let bytes = std::fs::metadata(&out).unwrap().len();
    println!("atlas {width}x{height}; camera {camera:?}; wrote {out}: {:.1} MiB ({} vertices of 20 B)", bytes as f64 / 1048576.0, total * 4);

    // The atlas as a picture beside the pack, to check it by eye.
    let preview = Path::new(&out).with_extension("atlas.png");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&preview).expect("preview file")), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&atlas).unwrap();
    println!("atlas preview: {}", preview.display());
}
