//! Horizon controller cursor artwork.

/// The atlas sprite.
pub const SPRITE: &str = "horizon:gui/controller/crosshair_inv";
/// The size it's drawn at in GUI pixels (an item's); its texture is twice that.
pub const DRAWN: f32 = 16.0;

#[doc(hidden)]
pub const PNG: &[u8] =
    include_bytes!("../../horizon-client/assets/horizon/textures/gui/controller/crosshair_inv.png");

/// Where to draw the cursor (top-left, GUI pixels) so its centre lands on
/// the GUI point `at`.
pub fn origin(at: (f32, f32)) -> (f32, f32) {
    (at.0 - DRAWN / 2.0, at.1 - DRAWN / 2.0)
}
