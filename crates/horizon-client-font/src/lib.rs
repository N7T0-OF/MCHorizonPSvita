//! Horizon's CPU-side font loading, rasterization and text ordering.
#![forbid(unsafe_code)]

pub mod bidi;
pub mod font;
pub mod image;
pub mod title_edition;

use horizon_client_core::{lang, text};
use horizon_core::fast_hash;
use horizon_resource_zip as zip;

// Keep the original font module's atlas and PNG dependencies on shared CPU
// data while the renderer retains its public `hud` paths.
mod hud {
    pub use crate::image::decode;
    pub use horizon_client_core::ui_geometry::Rect;
}
