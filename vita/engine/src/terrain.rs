//! Portable terrain for the renderer bring-up.
//!
//! The heights come from Horizon's own Perlin sampler (`horizon-core`). The columns and
//! the mesher below are a stand-in written for this probe: they are NOT Horizon's world
//! generator or `horizon-terrain-mesh`, which need the prepared game resources.

use horizon_core::noise::PerlinNoise;
use horizon_core::random::XoroshiroRandom;

pub const CHUNK: usize = 16;
pub const CHUNKS_PER_SIDE: usize = 8;
pub const SIDE: usize = CHUNK * CHUNKS_PER_SIDE;
pub const SEA: i32 = 20;
const MAX_HEIGHT: i32 = 56;

/// What vita2d's colour shader reads: position, then RGBA bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub position: [f32; 3],
    pub colour: u32,
}

pub struct ChunkMesh {
    pub vertices: Vec<Vertex>,
    /// Indices into `vertices`, three per triangle.
    pub indices: Vec<u16>,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

pub struct World {
    /// Solid height of each column, `SIDE * SIDE`, row by row.
    heights: Vec<i32>,
}

impl World {
    pub fn new(seed: i64) -> Self {
        let mut random = XoroshiroRandom::new(seed);
        let octaves = [PerlinNoise::new(&mut random), PerlinNoise::new(&mut random), PerlinNoise::new(&mut random)];
        let mut heights = Vec::with_capacity(SIDE * SIDE);
        for z in 0..SIDE {
            for x in 0..SIDE {
                let (fx, fz) = (x as f64 / 48.0, z as f64 / 48.0);
                let mut value = 0.0f32;
                let mut scale = 1.0f64;
                for octave in &octaves {
                    value += octave.get2(fx * scale, fz * scale) / scale as f32;
                    scale *= 2.0;
                }
                heights.push((SEA + 2 + (value * 26.0) as i32).clamp(1, MAX_HEIGHT));
            }
        }
        World { heights }
    }

    fn ground(&self, x: i32, z: i32) -> i32 {
        if x < 0 || z < 0 || x >= SIDE as i32 || z >= SIDE as i32 {
            return 0;
        }
        self.heights[z as usize * SIDE + x as usize]
    }

    /// The visible top of a column: the ground, or the water above it.
    fn surface(&self, x: i32, z: i32) -> i32 {
        let ground = self.ground(x, z);
        if ground == 0 { 0 } else { ground.max(SEA) }
    }

    pub fn surface_at(&self, x: f32, z: f32) -> f32 {
        self.surface(x as i32, z as i32) as f32
    }

    /// FNV-1a over the heights: the same on every correct target.
    pub fn checksum(&self) -> u32 {
        let mut hash: u32 = 0x811c_9dc5;
        for height in &self.heights {
            for byte in height.to_le_bytes() {
                hash = (hash ^ byte as u32).wrapping_mul(0x0100_0193);
            }
        }
        hash
    }

    pub fn mesh_chunk(&self, cx: usize, cz: usize) -> ChunkMesh {
        let mut mesh = ChunkMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            min: [(cx * CHUNK) as f32, 0.0, (cz * CHUNK) as f32],
            max: [((cx + 1) * CHUNK) as f32, 0.0, ((cz + 1) * CHUNK) as f32],
        };
        for z in (cz * CHUNK) as i32..((cz + 1) * CHUNK) as i32 {
            for x in (cx * CHUNK) as i32..((cx + 1) * CHUNK) as i32 {
                let ground = self.ground(x, z);
                let top = self.surface(x, z);
                let (fx, fz, fy) = (x as f32, z as f32, top as f32);
                mesh.max[1] = mesh.max[1].max(fy);
                let (top_colour, side_colour) = palette(ground, x, z);
                mesh.quad([[fx, fy, fz], [fx, fy, fz + 1.0], [fx + 1.0, fy, fz + 1.0], [fx + 1.0, fy, fz]], shade(top_colour, 255));
                // A wall down to each lower neighbour, as one quad: with flat colours a
                // stack of block faces would look the same and cost more vertices.
                for (dx, dz, light) in [(-1, 0, 150u32), (1, 0, 150), (0, -1, 200), (0, 1, 200)] {
                    let low = self.surface(x + dx, z + dz);
                    if low >= top {
                        continue;
                    }
                    let (fl, colour) = (low as f32, shade(side_colour, light));
                    let corners = match (dx, dz) {
                        (-1, 0) => [[fx, fy, fz], [fx, fl, fz], [fx, fl, fz + 1.0], [fx, fy, fz + 1.0]],
                        (1, 0) => [[fx + 1.0, fy, fz + 1.0], [fx + 1.0, fl, fz + 1.0], [fx + 1.0, fl, fz], [fx + 1.0, fy, fz]],
                        (0, -1) => [[fx + 1.0, fy, fz], [fx + 1.0, fl, fz], [fx, fl, fz], [fx, fy, fz]],
                        _ => [[fx, fy, fz + 1.0], [fx, fl, fz + 1.0], [fx + 1.0, fl, fz + 1.0], [fx + 1.0, fy, fz + 1.0]],
                    };
                    mesh.quad(corners, colour);
                }
            }
        }
        mesh
    }
}

impl ChunkMesh {
    fn quad(&mut self, corners: [[f32; 3]; 4], colour: u32) {
        let base = self.vertices.len() as u16;
        for position in corners {
            self.vertices.push(Vertex { position, colour });
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// (top, side) colours of a column, as (r, g, b).
fn palette(ground: i32, x: i32, z: i32) -> ((u32, u32, u32), (u32, u32, u32)) {
    let speckle = ((x * 7 + z * 13) & 3) as u32 * 5;
    if ground < SEA {
        ((40, 96, 200), (40, 96, 200))
    } else if ground <= SEA + 1 {
        ((214, 200, 140), (196, 182, 124))
    } else if ground > SEA + 24 {
        ((236, 240, 246), (128, 128, 134))
    } else if ground > SEA + 17 {
        ((128 + speckle, 128 + speckle, 134), (112, 112, 118))
    } else {
        ((66 + speckle, 150 + speckle, 58), (124, 88, 60))
    }
}

/// 0xAABBGGRR, the byte order the shader's colour attribute reads.
fn shade((r, g, b): (u32, u32, u32), light: u32) -> u32 {
    0xFF00_0000 | (b * light / 255) << 16 | (g * light / 255) << 8 | (r * light / 255)
}

/// The Rust probe's self-test: three octaves of Horizon's Perlin noise over a 240x136
/// grid, as an FNV-1a checksum of the samples' bits. A console that computes
/// `horizon-core` exactly like the PC gives the same value.
pub fn noise_checksum(seed: i64) -> u32 {
    let mut random = XoroshiroRandom::new(seed);
    let octaves = [PerlinNoise::new(&mut random), PerlinNoise::new(&mut random), PerlinNoise::new(&mut random)];
    let mut checksum: u32 = 0x811c_9dc5;
    for z in 0..136 {
        for x in 0..240 {
            let (fx, fz) = (x as f64 / 48.0, z as f64 / 48.0);
            let mut value = 0.0f32;
            let mut scale = 1.0f64;
            for octave in &octaves {
                value += octave.get2(fx * scale, fz * scale) / scale as f32;
                scale *= 2.0;
            }
            for byte in value.to_bits().to_le_bytes() {
                checksum = (checksum ^ byte as u32).wrapping_mul(0x0100_0193);
            }
        }
    }
    checksum
}
