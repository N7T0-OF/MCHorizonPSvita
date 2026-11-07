//! Winit-backed desktop window operations. Shared input/window state stays in
//! `window`; this module translates it to the desktop window system.

use crate::window::{FullscreenState, VideoMode};

/// Convert a Winit monitor mode to Horizon's saved display-mode representation.
pub fn video_mode_from_winit(mode: &winit::monitor::VideoModeHandle) -> VideoMode {
    let depth = i32::from(mode.bit_depth());
    let size = mode.size();
    VideoMode {
        width: size.width as i32,
        height: size.height as i32,
        red_bits: depth / 3,
        green_bits: depth / 3,
        blue_bits: depth - 2 * (depth / 3),
        refresh_rate: mode.refresh_rate_millihertz() as f32 / 1000.0,
    }
}

impl VideoMode {
    /// Convert a Winit display mode into Horizon's saved display-mode representation.
    ///
    /// This compatibility wrapper is available with the `desktop` feature; the
    /// platform-neutral `VideoMode` definition remains free of Winit types.
    pub fn from_winit(mode: &winit::monitor::VideoModeHandle) -> Self {
        video_mode_from_winit(mode)
    }
}

/// `Monitor.videoModes`: the modes a monitor offers.
pub fn monitor_video_modes(monitor: &winit::monitor::MonitorHandle) -> Vec<VideoMode> {
    monitor
        .video_modes()
        .map(|m| video_mode_from_winit(&m))
        .collect()
}

/// `Monitor.currentMode`: Winit has no direct query, so use the monitor's own
/// size with the highest refresh rate and depth.
fn monitor_current_mode(
    monitor: &winit::monitor::MonitorHandle,
) -> Option<winit::monitor::VideoModeHandle> {
    let size = monitor.size();
    monitor
        .video_modes()
        .filter(|m| m.size() == size)
        .max_by_key(|m| (m.refresh_rate_millihertz(), m.bit_depth()))
}

/// `Window.applyFullscreen`: exclusive mode uses the preferred mode when
/// available, else the desktop mode; without a usable mode, use borderless.
pub fn fullscreen_request(
    window: &winit::window::Window,
    exclusive: bool,
    preferred: Option<&VideoMode>,
) -> winit::window::Fullscreen {
    use winit::window::Fullscreen;
    if !exclusive {
        return Fullscreen::Borderless(None);
    }
    let Some(monitor) = window
        .current_monitor()
        .or_else(|| window.primary_monitor())
    else {
        return Fullscreen::Borderless(None);
    };
    let wanted = preferred.and_then(|p| {
        monitor
            .video_modes()
            .find(|m| video_mode_from_winit(m) == *p)
    });
    match wanted.or_else(|| monitor_current_mode(&monitor)) {
        Some(mode) => Fullscreen::Exclusive(mode),
        None => Fullscreen::Borderless(None),
    }
}

impl FullscreenState {
    /// Apply the platform-neutral fullscreen transition to a Winit window.
    /// A changed preferred mode is held while the Video Settings screen is open.
    pub fn apply(
        &mut self,
        window: &winit::window::Window,
        wanted: bool,
        exclusive: bool,
        preferred: Option<VideoMode>,
        in_video_screen: bool,
    ) {
        let request = self.request(wanted);
        let reapply = self.set_exclusive(exclusive);
        self.set_preferred_mode(preferred);
        let mode_change = !in_video_screen && self.take_mode_change();
        match request {
            Some(false) => window.set_fullscreen(None),
            Some(true) => window.set_fullscreen(Some(fullscreen_request(
                window,
                exclusive,
                preferred.as_ref(),
            ))),
            None if reapply || mode_change => window.set_fullscreen(Some(fullscreen_request(
                window,
                exclusive,
                preferred.as_ref(),
            ))),
            None => {}
        }
    }
}

/// Horizon's title-bar/taskbar icon; Wayland takes it from the desktop entry
/// and macOS from the app bundle.
pub fn icon() -> Option<winit::window::Icon> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(horizon_theme::ICON_PNG));
    decoder.ignore_checksums(true);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut bytes = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut bytes).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    bytes.truncate(info.buffer_size());
    winit::window::Icon::from_rgba(bytes, info.width, info.height).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_icon_decodes() {
        assert!(icon().is_some());
    }
}
