//! CPU-side atlas and font geometry shared by client presentation backends.

/// A sprite's atlas region, and its size in GUI pixels: the size of the
/// default resources' texture of that name, whatever the resolution of the
/// pack that replaced it (vanilla's GUI code draws in those units: `blit`
/// divides by the texture size it is given, sprites by their `gui.scaling`
/// size). Textures the default resources lack keep their own size.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub w: u32,
    pub h: u32,
}
impl Rect {
    /// The region's size in texels of an atlas this large.
    pub fn texels(&self, (width, height): (u32, u32)) -> (u32, u32) {
        (
            ((self.u1 - self.u0) * width as f32).round() as u32,
            ((self.v1 - self.v0) * height as f32).round() as u32,
        )
    }
}

/// A glyph ready to draw: atlas UVs and its box relative to the pen
/// position (GlyphBitmap left/top/right/bottom), in GUI pixels.
#[derive(Clone, Copy, Debug, Default)]
pub struct Glyph {
    pub rect: Option<Rect>,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub advance: f32,
    /// Shadow and bold offset (1 for bitmap and TrueType glyphs, 0.5 for unihex).
    pub offset: f32,
}
