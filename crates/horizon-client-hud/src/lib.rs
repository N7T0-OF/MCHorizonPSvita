//! Horizon's existing CPU HUD atlas and batched drawing data.

pub use horizon_client_core::Result;
#[cfg(test)]
use horizon_client_core::component;
use horizon_client_core::{player_profile as skins, player_profile as tab_list, text};
use horizon_client_font::{bidi, font, pad_art as pad_glyph, title_edition, trim_slots as trims};
use horizon_hud_data::minimap;
use horizon_shaders::{core_lines_glue, pack_text};
use horizon_terrain_mesh::{atlas_sources, cit_armor, cit_assets, cit_assets as cit, par, sprites};

pub mod sprite_animations;
mod hud;
pub use hud::*;
