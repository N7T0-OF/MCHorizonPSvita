//! Horizon's shared CPU-side UI widgets and input presentation.
#![forbid(unsafe_code)]

pub mod pad_glyph;
pub mod preedit;
pub mod ui;
pub mod widgets;

pub use horizon_client_core::text;
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
