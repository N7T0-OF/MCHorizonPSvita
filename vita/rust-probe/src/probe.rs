//! Portable part of the probe: Horizon's own Perlin sampler from `horizon-core`,
//! driven the same way on the PC and on the console so the two can be compared.

use horizon_core::noise::PerlinNoise;
use horizon_core::random::XoroshiroRandom;

pub const MAP_W: usize = 240;
pub const MAP_H: usize = 136;

pub struct NoiseMap {
    /// `MAP_W * MAP_H` samples, row by row.
    pub samples: Vec<f32>,
    /// FNV-1a over the samples' bit patterns: identical on every correct target.
    pub checksum: u32,
}

/// Samples three octaves of Horizon's Perlin noise over a `MAP_W` x `MAP_H` grid.
pub fn noise_map(seed: i64) -> NoiseMap {
    let mut random = XoroshiroRandom::new(seed);
    let octaves = [PerlinNoise::new(&mut random), PerlinNoise::new(&mut random), PerlinNoise::new(&mut random)];
    let mut samples = Vec::with_capacity(MAP_W * MAP_H);
    let mut checksum: u32 = 0x811c_9dc5;
    for z in 0..MAP_H {
        for x in 0..MAP_W {
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
            samples.push(value);
        }
    }
    NoiseMap { samples, checksum }
}

/// A terrain-like colour for one sample (0xAABBGGRR, the Vita framebuffer's order).
pub fn colour(value: f32) -> u32 {
    let (r, g, b) = if value < -0.15 {
        (24, 60, 150)
    } else if value < 0.0 {
        (40, 100, 200)
    } else if value < 0.05 {
        (214, 200, 140)
    } else if value < 0.45 {
        let shade = (value * 160.0) as u32;
        (50 + shade / 2, 140 - shade / 2, 50)
    } else {
        (150, 150, 155)
    };
    0xFF00_0000 | (b << 16) | (g << 8) | r
}
