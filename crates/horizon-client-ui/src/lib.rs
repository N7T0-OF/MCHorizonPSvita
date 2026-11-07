//! Horizon's shared CPU-side UI widgets and input presentation.
#![forbid(unsafe_code)]

pub mod confirm_screen;
pub mod datapack;
pub mod dialog_screen;
pub mod game_rules_screen;
pub mod hud_layout;
pub mod pad_glyph;
pub mod preedit;
pub mod screen_draw;
pub mod screen_narrator;
pub mod title_confirm_experiments;
pub mod title_experiments;
pub mod toast_slide;
pub mod ui;
pub mod widgets;

pub use horizon_client_core::{component, dialog, gamerules, lang, text};
pub use horizon_client_font::{bidi, font};
pub use horizon_client_input::{clipboard, gamepad};

pub mod hud {
    pub use horizon_client_hud::*;
}

// Keep one origin shared by the original UI and HUD overlay callers.
pub mod hud_overlays {
    pub use crate::now_ms;
}

/// Milliseconds on a monotonic clock (`Util.getMillis`).
pub fn now_ms() -> i64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as i64
}
