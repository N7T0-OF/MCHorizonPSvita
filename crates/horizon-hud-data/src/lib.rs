#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub mod minimap {
    /// The minimap image's side in texels.
    pub const SIZE: usize = 128;
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: u32,
    pub depth: f32,
    /// The `GLINT_TEXTURING` UV into the repeating glint texture, unused
    /// unless `uv.x >= 2`.
    pub glint_uv: [f32; 2],
    /// 1 for the GUI's text (`RenderPipelines.GUI_TEXT`), which a resource
    /// pack's text shader draws when it has one.
    pub text: u32,
}
/// How a run of HUD vertices blends and tests depth.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Blend {
    /// `BlendFunction.TRANSLUCENT`.
    Alpha,
    /// `BlendFunction.INVERT` (`RenderPipelines.CROSSHAIR`, `GUI_INVERT`).
    Invert,
    /// `BlendFunction.ADDITIVE` (`RenderPipelines.GUI_TEXT_HIGHLIGHT`).
    Additive,
    /// Alpha blending with an isolated depth buffer and a pixel scissor.
    Depth([u32; 4]),
    /// `RenderPipelines.VIGNETTE`: the destination darkened by the texture
    /// (`ZERO, ONE_MINUS_SRC_COLOR`).
    Vignette,
    /// `RenderPipelines.END_PORTAL` as a GUI fill: the End portal's layers
    /// projected from the screen (`HudBatch::end_portal` sets the textures).
    EndPortal,
    /// An OptiFine CIT glint layer (`cit_glint`), blended as entry `n` of
    /// `GlintTextures::blends` says.
    CitGlint(u8),
}
/// What a `Blend::EndPortal` fill samples: the atlas rects (u0, v0, width,
/// height) of `end_sky.png` and `end_portal.png`, and the `GameTime` uniform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EndPortalFill {
    pub sprites: [[f32; 4]; 2],
    pub game_time: f32,
}
pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    u32::from(a) << 24 | u32::from(b) << 16 | u32::from(g) << 8 | u32::from(r)
}

/// Pixels to copy into the shared atlas before the next HUD draw.
#[derive(Clone, Debug)]
pub struct Patch {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

// Checked on every target, including Cafe: the GX2 bridge consumes this layout.
const _: () = {
    assert!(core::mem::size_of::<HudVertex>() == 36);
    assert!(core::mem::align_of::<HudVertex>() == 4);
    assert!(core::mem::offset_of!(HudVertex, pos) == 0);
    assert!(core::mem::offset_of!(HudVertex, uv) == 8);
    assert!(core::mem::offset_of!(HudVertex, color) == 16);
    assert!(core::mem::offset_of!(HudVertex, depth) == 20);
    assert!(core::mem::offset_of!(HudVertex, glint_uv) == 24);
    assert!(core::mem::offset_of!(HudVertex, text) == 32);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_color_keeps_horizons_channel_order() {
        assert_eq!(rgba(0x12, 0x34, 0x56, 0x78), 0x7856_3412);
        let vertex = HudVertex {
            pos: [1.0, 2.0],
            uv: [0.25, 0.5],
            color: rgba(0x12, 0x34, 0x56, 0x78),
            depth: 0.75,
            glint_uv: [0.0, 1.0],
            text: 1,
        };
        let bytes = bytemuck::bytes_of(&vertex);
        assert_eq!(&bytes[16..20], &0x7856_3412u32.to_ne_bytes());
    }
}
