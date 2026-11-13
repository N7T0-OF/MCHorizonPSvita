//! Packs a region of a real Horizon world for the PS Vita to draw.
//!
//! Everything about the world is Horizon's own code running on the PC, through
//! `vita/world`: the 26.3 overworld generator (terrain, carvers, structures, then the
//! FEATURES decoration: trees, ores, plants) and the canonical terrain mesher with the
//! prepared block models and sprites. The console only draws the result.
//!
//! What this file adds is the reduction to what the console can draw today, with
//! libvita2d's precompiled texture shader (a texture, no vertex colours):
//!   - each quad's tint and face shading is baked into its own atlas cell, so the
//!     colours are per quad (no smooth lighting);
//!   - sprites are reduced to 16 px;
//!   - every block is fully sky-lit: Horizon's light engine is not wired in.
//!
//! The pack holds textures and terrain derived from the user's own copy of Minecraft:
//! it stays on their PC and console and must never be committed or shared.
//!
//! usage: pack <prepared-26.3-resources> <out.hvw> [seed] [radius]

use horizon_vita_world::{Horizon, TerrainVertex, OPAQUE_LAYER};
use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;
use std::time::Instant;

/// Pixels of a sprite in the console's atlas, and of its cell with a 1 px border.
const TEXELS: usize = 16;
const CELL: usize = TEXELS + 2;
/// The console draws quads with a shared 16-bit index pattern: 4 vertices each.
const MAX_BATCH_QUADS: usize = 16384;

const OPAQUE: u8 = 0;
const CUTOUT: u8 = 1;
const TRANSLUCENT: u8 = 2;

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
    let mut horizon = Horizon::load(root, seed, |_| {}).expect("Horizon");
    let spawn = horizon.spawn;
    println!("Horizon loaded in {:.1} s; seed {seed}, spawn {spawn:?}", start.elapsed().as_secs_f32());

    let start = Instant::now();
    let centre = (spawn[0].div_euclid(16), spawn[1].div_euclid(16));
    let (region, decorated, surface) = horizon.generate(centre, radius, true).expect("generation");
    let surface = surface.expect("the spawn column is in the region");
    println!(
        "generated {} chunks and decorated {} in {:.1} s: {} features placed, {} unsupported, {} failed",
        (2 * radius + 5) * (2 * radius + 5),
        decorated.chunks,
        start.elapsed().as_secs_f32(),
        decorated.placed,
        decorated.unsupported,
        decorated.failed
    );
    if !decorated.skipped.is_empty() {
        println!("  skipped features: {}", decorated.skipped.join(", "));
    }

    // Sprite facts for the atlas.
    let (sprite, layers, pixels) = horizon.sprites();
    assert_eq!(pixels.len(), layers * sprite * sprite * 4, "sprite pixels");
    let has_alpha: Vec<bool> = (0..layers).map(|layer| pixels[layer * sprite * sprite * 4..][..sprite * sprite * 4].chunks_exact(4).any(|p| p[3] < 255)).collect();

    let start = Instant::now();
    let mut cells: HashMap<CellKey, u32> = HashMap::new();
    let mut cell_keys: Vec<CellKey> = Vec::new();
    let mut batches: Vec<Batch> = Vec::new();
    let mut counts = [0usize; 3];
    // Packed coordinates start at the packed area's corner, so they stay small.
    let packed_origin = (centre.0 - radius, centre.1 - radius);
    for cz in centre.1 - radius..=centre.1 + radius {
        for cx in centre.0 - radius..=centre.0 + radius {
            // One batch per section and mode, filled as the mesher hands quads over.
            let mut sections: Vec<[Vec<Quad>; 3]> = (0..region.sections()).map(|_| Default::default()).collect();
            // Flat lighting: one colour per quad, which is what an atlas cell can hold.
            horizon
                .mesh(&region, (cx, cz), false, |section, quad, translucent| {
                    let base = [((cx - packed_origin.0) * 16) as f32, (region.min_y + section as i32 * 16) as f32, ((cz - packed_origin.1) * 16) as f32];
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
                    sections[section][mode as usize].push(Quad {
                        positions: std::array::from_fn(|i| std::array::from_fn(|axis| quad[i].pos[axis] + base[axis])),
                        uvs: std::array::from_fn(|i| TerrainVertex::uv(&quad[i]).map(|v| v.clamp(0.0, 1.0))),
                        cell,
                    });
                })
                .expect("meshing");
            for modes in sections {
                for (mode, mut quads) in modes.into_iter().enumerate() {
                    while !quads.is_empty() {
                        let rest = quads.split_off(quads.len().min(MAX_BATCH_QUADS));
                        batches.push(Batch { mode: mode as u8, quads });
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
