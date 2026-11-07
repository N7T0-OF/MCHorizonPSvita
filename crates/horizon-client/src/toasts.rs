//! `ToastManager`'s slide, shared by the game's toasts and the title
//! screens' system toasts.
use crate::hud::{HudAtlas, HudBatch};
use std::time::Instant;

pub use horizon_client_render::toast_slide::Slide;

pub use horizon_client_render::toast_slide::{SystemToast, background};

/// `NowPlayingToast`'s `musicNoteColorTick`, `lastMusicNoteColorChange` and
/// `musicNoteColor`: the notes change colour every 25 ms, while a song plays.
pub struct MusicNotes {
    tick: i32,
    last_change: Option<Instant>,
    /// Opaque ARGB, white until the first change.
    pub color: u32,
}
impl Default for MusicNotes {
    fn default() -> Self {
        Self {
            tick: 0,
            last_change: None,
            color: u32::MAX,
        }
    }
}
impl MusicNotes {
    /// `NowPlayingToast.tickMusicNotes`, for a song that is playing.
    pub fn tick(&mut self) {
        if self
            .last_change
            .is_none_or(|at| at.elapsed().as_millis() > 25)
        {
            self.tick += 1;
            self.last_change = Some(Instant::now());
            self.color = music_note_color(self.tick);
        }
    }
}
/// `ColorLerper.getLerpedColor(Type.MUSIC_NOTE, tick)` as opaque ARGB: the
/// dye colours of `MUSIC_NOTE_COLORS` at 125 % (white is 0xE6E6E6), one per
/// 30 ticks, lerped in between.
fn music_note_color(tick: i32) -> u32 {
    // DyeColor ids of WHITE, LIGHT_GRAY, LIGHT_BLUE, BLUE, CYAN, GREEN, LIME,
    // YELLOW, ORANGE, PINK, RED and MAGENTA.
    const DYES: [usize; 12] = [0, 8, 3, 11, 9, 13, 5, 4, 1, 6, 14, 2];
    let rgb = |dye: usize| -> [i32; 3] {
        if dye == 0 {
            return [0xE6; 3];
        }
        let src = crate::mobs::render::DYE[dye];
        [16, 8, 0].map(|shift| (((src >> shift & 255) as f32 * 1.25).floor() as i32).clamp(0, 255))
    };
    let value = (tick / 30) as usize;
    let (from, to) = (
        rgb(DYES[value % DYES.len()]),
        rgb(DYES[(value + 1) % DYES.len()]),
    );
    let sub = (tick % 30) as f32 / 30.0;
    // ARGB.srgbLerp: Mth.lerpInt per channel.
    let lerp = |i: usize| (from[i] + (sub * (to[i] - from[i]) as f32).floor() as i32) as u32;
    0xFF00_0000 | lerp(0) << 16 | lerp(1) << 8 | lerp(2)
}

/// `NowPlayingToast` in its `ToastManager.ToastInstance`: it slides in from
/// the left edge at the top of the screen and goes out after five seconds
/// (scaled by the Notification Time option).
#[derive(Default)]
pub struct NowPlayingToast {
    slide: Slide,
    /// `updateToast`: `showToast` sets it, `onFinishedRendering` clears it.
    update_toast: bool,
    multiplier: f64,
    /// `wantedVisibility` is SHOW.
    wanted: bool,
    finished: bool,
}
impl NowPlayingToast {
    /// `ToastManager.showNowPlayingToast`: `resetToast`, then `showToast`.
    pub fn show(&mut self, multiplier: f64) {
        self.slide = Slide::default();
        self.finished = false;
        self.update_toast = true;
        self.multiplier = multiplier;
        self.wanted = true;
    }
    /// `ToastManager.hideNowPlayingToast`.
    pub fn hide(&mut self) {
        self.wanted = false;
    }
    /// `ToastInstance.update` with `NowPlayingToast.update`. `ToastManager`
    /// plays no visibility sound for this toast.
    pub fn update(&mut self, song_playing: bool, notes: &mut MusicNotes) {
        let (update_toast, multiplier) = (self.update_toast, self.multiplier);
        let wanted = &mut self.wanted;
        let (_, finished) = self.slide.update_now(|fully| {
            if update_toast {
                *wanted = (fully as f64) < 5000.0 * multiplier;
                if song_playing {
                    notes.tick();
                }
            }
            *wanted
        });
        if finished && !self.finished {
            self.update_toast = false;
        }
        self.finished = finished;
    }
    /// `ToastInstance.extractRenderState` with `Toast.xPos`: nothing once it
    /// has slid out.
    pub fn draw(&self, atlas: &HudAtlas, batch: &mut HudBatch, song: &str, notes: &MusicNotes) {
        if self.finished {
            return;
        }
        let width = Self::width(atlas, song);
        draw_now_playing(
            atlas,
            batch,
            width * self.slide.portion - width,
            song,
            notes.color,
        );
    }
    /// `NowPlayingToast.getWidth`.
    fn width(atlas: &HudAtlas, song: &str) -> f32 {
        30.0 + HudBatch::text_width(atlas, song).ceil() + 7.0
    }
}
/// `NowPlayingToast.extractToast` at `x` on the top row: the background, the
/// music notes in `notes` (ARGB) and the song's name in light grey.
pub fn draw_now_playing(atlas: &HudAtlas, batch: &mut HudBatch, x: f32, song: &str, notes: u32) {
    let width = NowPlayingToast::width(atlas, song);
    // `toast/now_playing`: a nine slice of 160 by 32 with a border of 4, here 30 high.
    if let Some(rect) = atlas.sprites.get("gui/sprites/toast/now_playing").copied() {
        let (du, dv) = (
            (rect.u1 - rect.u0) / rect.w as f32,
            (rect.v1 - rect.v0) / rect.h as f32,
        );
        let source_x = [0.0, 4.0, rect.w as f32 - 4.0, rect.w as f32];
        let source_y = [0.0, 4.0, rect.h as f32 - 4.0, rect.h as f32];
        let target_x = [x, x + 4.0, x + width - 4.0, x + width];
        let target_y = [0.0, 4.0, 26.0, 30.0];
        for row in 0..3 {
            for col in 0..3 {
                let piece = crate::hud::Rect {
                    u0: rect.u0 + source_x[col] * du,
                    u1: rect.u0 + source_x[col + 1] * du,
                    v0: rect.v0 + source_y[row] * dv,
                    v1: rect.v0 + source_y[row + 1] * dv,
                    ..rect
                };
                batch.rect_uv(
                    target_x[col],
                    target_y[row],
                    target_x[col + 1] - target_x[col],
                    target_y[row + 1] - target_y[row],
                    piece,
                    u32::MAX,
                );
            }
        }
    }
    if let Some(rect) = atlas.sprites.get("gui/sprites/icon/music_notes").copied() {
        batch.rect_uv(
            x + 7.0,
            7.0,
            16.0,
            16.0,
            rect,
            crate::mobs::render::abgr(notes & 0x00FF_FFFF),
        );
    }
    // DyeColor.LIGHT_GRAY.getTextColor, at 15 - 9 / 2.
    batch.text(atlas, song, x + 30.0, 11.0, 0xFFD3_D3D3, true);
}
