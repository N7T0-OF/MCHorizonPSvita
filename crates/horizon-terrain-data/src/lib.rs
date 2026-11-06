#![no_std]

/// A section vertex: position, packed UV, sprite layer, smooth light, and
/// RGBA8 colour. Its 24-byte layout is shared by Horizon's renderer and
/// embedded backends that adapt the same mesher output.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainVertex {
    /// Section-relative position.
    pub pos: [f32; 3],
    /// UV in fixed point, with [`UV_STEPS`] units per sprite coordinate.
    pub uv: [u16; 2],
    /// Sprite array layer, with Horizon's opaque-layer flag where applicable.
    pub layer: u16,
    /// Smooth light: sky in the low byte, block in the high byte.
    pub light: u16,
    /// RGBA8: red in the low byte, alpha in the high byte.
    pub color: u32,
}

/// Fixed-point steps per unit of `TerrainVertex` UVs, which span `[0, 2)`.
pub const UV_STEPS: f32 = 32768.0;
/// Fixed-point steps per block of distant-terrain UVs, which span `[0, 2048)`.
pub const TILED_UV_STEPS: f32 = 32.0;

/// `v.round().clamp(0.0, 65535.0) as u16`, with clamping before conversion.
#[inline]
fn round_to_u16(v: f32) -> u16 {
    let v = v.clamp(0.0, f32::from(u16::MAX));
    let whole = v as u32;
    (whole + u32::from(v - whole as f32 >= 0.5)) as u16
}

impl TerrainVertex {
    pub fn new(pos: [f32; 3], uv: [f32; 2], layer: u16, color: u32, light: u16) -> Self {
        Self {
            pos,
            uv: uv.map(|v| round_to_u16(v * UV_STEPS)),
            layer,
            light,
            color,
        }
    }

    /// A vertex of a merged distant-terrain face; UVs are in blocks from its corner.
    pub fn tiled(pos: [f32; 3], uv: [f32; 2], layer: u16, color: u32, light: u16) -> Self {
        Self {
            pos,
            uv: uv.map(|v| round_to_u16(v * TILED_UV_STEPS)),
            layer,
            light,
            color,
        }
    }

    /// The UV the GPU samples.
    pub fn uv(&self) -> [f32; 2] {
        self.uv.map(|v| f32::from(v) / UV_STEPS)
    }

    /// Unpacks the vertex's little-endian RGBA8 value independent of CPU byte order.
    pub fn rgba(&self) -> [u8; 4] {
        [
            self.color as u8,
            (self.color >> 8) as u8,
            (self.color >> 16) as u8,
            (self.color >> 24) as u8,
        ]
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::TerrainVertex;
    use std::vec;

    #[test]
    fn model_uvs_pack_exactly() {
        for i in 0..=32 {
            let u = i as f32 / 32.0;
            let vertex = TerrainVertex::new([0.0; 3], [u, 1.0 - u], 0, 0, 0);
            assert_eq!(vertex.uv(), [u, 1.0 - u]);
        }
    }

    #[test]
    fn uv_rounding_matches_f32_round() {
        let reference = |v: f32| v.round().clamp(0.0, f32::from(u16::MAX)) as u16;
        let mut values = vec![
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -0.0,
            0.5,
            1.5,
            65534.5,
            1e9,
        ];
        for bits in (0..u32::MAX).step_by(9973) {
            values.push(f32::from_bits(bits));
        }
        for i in 0..=140_000 {
            values.push(i as f32 * 0.5 - 2.0);
            values.push(i as f32 * 0.5 - 2.0 + f32::EPSILON);
        }
        for value in values {
            assert_eq!(super::round_to_u16(value), reference(value), "{value:e}");
        }
    }

    #[test]
    fn rgba_unpack_is_independent_of_host_byte_order() {
        let vertex = TerrainVertex::new([1.0, 2.0, 3.0], [0.5, 1.5], 7, 0xA1B2_C3D4, 0x1234);
        assert_eq!(vertex.rgba(), [0xD4, 0xC3, 0xB2, 0xA1]);
        assert_eq!(core::mem::size_of::<TerrainVertex>(), 24);
    }

    #[test]
    fn non_grid_uvs_round_within_half_a_step() {
        let uv = [0.5 + 0.25 * 0.3f32.sin(), 0.5 - 0.25 * 0.3f32.cos()];
        let packed = TerrainVertex::new([0.0; 3], uv, 0, 0, 0).uv();
        for (actual, expected) in packed.iter().zip(uv) {
            assert!((actual - expected).abs() <= 0.5 / 32768.0);
        }
    }
}
