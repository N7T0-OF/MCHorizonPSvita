//! A controller button as the player sees it: Horizon's own art where the pad's family has one
//! (`assets/horizon/textures/gui/controller/`, which a resource pack can replace), else the label
//! printed on the pad, in its colour, on a dark key. Whatever shows a button draws and measures it
//! here; `Button::label` stays for plain text and narration.
use crate::gamepad::{Button, GlyphStyle};
use crate::hud::{HudAtlas, HudBatch, rgba};
pub use horizon_client_font::pad_art::{FACE_WIDTH, HEIGHT, images, sprite};
use horizon_client_font::pad_art::{MARGIN, art_width, find};

/// The sprite this atlas can draw for the button; none (the boot screen's atlas) means text.
pub fn art(atlas: &HudAtlas, button: Button, style: GlyphStyle) -> Option<&'static str> {
    sprite(button, style).filter(|name| atlas.sprites.contains_key(*name))
}

/// How wide the glyph shows.
pub fn width(atlas: &HudAtlas, button: Button, style: GlyphStyle) -> f32 {
    match find(button, style).filter(|a| atlas.sprites.contains_key(a.sprite)) {
        Some(art) => art_width(art),
        None => HudBatch::text_width(atlas, button.label(style)).ceil() + 4.0,
    }
}

/// What the glyph covers with its label's text at `(x, y)`: `[x, y, w, h]`.
pub fn rect(atlas: &HudAtlas, button: Button, style: GlyphStyle, x: f32, y: f32) -> [f32; 4] {
    [x - 1.0, y - 2.0, width(atlas, button, style), HEIGHT]
}

/// Draw the glyph over the 8 pixel text line at `(x, y)`, filling [`rect`]. A dark key backs text
/// only: the art brings its own.
pub fn draw(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    button: Button,
    style: GlyphStyle,
    x: f32,
    y: f32,
) {
    let [left, top, wide, tall] = rect(atlas, button, style, x, y);
    if let Some(name) = art(atlas, button, style) {
        batch.sprite_scaled(
            atlas,
            name,
            left - MARGIN,
            top - MARGIN,
            wide + 2.0 * MARGIN,
            tall + 2.0 * MARGIN,
            u32::MAX,
        );
    } else {
        let [r, g, b] = button.color(style);
        batch.fill(atlas, left, top, wide, tall, rgba(40, 40, 40, 255));
        batch.text(
            atlas,
            button.label(style),
            x + 1.0,
            y,
            rgba(r, g, b, 255),
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atlas() -> HudAtlas {
        HudAtlas::build(std::path::Path::new("/nonexistent/horizon-test-assets")).unwrap()
    }

    fn file(button: Button, style: GlyphStyle) -> Option<&'static str> {
        sprite(button, style).and_then(|name| name.strip_prefix("horizon:gui/controller/"))
    }

    /// The batch's quads as `[left, top, right, bottom]`.
    fn quads(batch: &HudBatch) -> Vec<[f32; 4]> {
        batch
            .vertices
            .as_chunks::<6>()
            .0
            .iter()
            .map(|quad| {
                let xs = quad.iter().map(|v| v.pos[0]);
                let ys = quad.iter().map(|v| v.pos[1]);
                [
                    xs.clone().fold(f32::MAX, f32::min),
                    ys.clone().fold(f32::MAX, f32::min),
                    xs.fold(f32::MIN, f32::max),
                    ys.fold(f32::MIN, f32::max),
                ]
            })
            .collect()
    }

    #[test]
    fn xbox_and_auto_show_the_letters_and_shoulders() {
        for style in [GlyphStyle::Xbox, GlyphStyle::Auto] {
            for (button, expected) in [
                (Button::South, "xb_a"),
                (Button::East, "xb_b"),
                (Button::West, "xb_x"),
                (Button::North, "xb_y"),
                (Button::LeftBumper, "xb_lb"),
                (Button::RightBumper, "xb_rb"),
                (Button::LeftTrigger, "xb_lt"),
                (Button::RightTrigger, "xb_rt"),
                (Button::Select, "xb_view"),
                (Button::Start, "xb_menu"),
            ] {
                assert_eq!(file(button, style), Some(expected));
            }
        }
    }

    #[test]
    fn playstation_shows_its_symbols_by_position_and_its_own_shoulders() {
        let style = GlyphStyle::PlayStation;
        assert_eq!(file(Button::South, style), Some("psx_cross"));
        assert_eq!(file(Button::East, style), Some("psx_circle"));
        assert_eq!(file(Button::West, style), Some("psx_square"));
        assert_eq!(file(Button::North, style), Some("psx_triangle"));
        assert_eq!(file(Button::LeftBumper, style), Some("psx_l1"));
        assert_eq!(file(Button::RightTrigger, style), Some("psx_r2"));
        assert_eq!(file(Button::Select, style), Some("psx_create"));
    }

    #[test]
    fn nintendo_shows_what_its_pad_prints_at_each_position() {
        let style = GlyphStyle::Nintendo;
        assert_eq!(file(Button::South, style), Some("nx_b"));
        assert_eq!(file(Button::East, style), Some("nx_a"));
        assert_eq!(file(Button::West, style), Some("nx_y"));
        assert_eq!(file(Button::North, style), Some("nx_x"));
        assert_eq!(file(Button::LeftTrigger, style), Some("nx_zl"));
        assert_eq!(file(Button::Start, style), Some("nx_plus"));
    }

    #[test]
    fn every_family_has_its_own_file_for_every_button() {
        for (style, prefix) in [
            (GlyphStyle::Xbox, "xb_"),
            (GlyphStyle::PlayStation, "psx_"),
            (GlyphStyle::Nintendo, "nx_"),
        ] {
            for button in Button::ALL {
                let name = file(button, style).unwrap();
                assert!(name.starts_with(prefix), "{button:?} {name}");
            }
        }
    }

    #[test]
    fn art_measures_from_its_shape_and_text_from_its_label() {
        let atlas = atlas();
        assert_eq!(width(&atlas, Button::South, GlyphStyle::Xbox), 12.0);
        assert_eq!(width(&atlas, Button::LeftBumper, GlyphStyle::Xbox), 20.0);
        assert_eq!(
            width(&atlas, Button::LeftBumper, GlyphStyle::PlayStation),
            20.0
        );
        assert_eq!(width(&atlas, Button::DPadUp, GlyphStyle::Nintendo), 12.0);
        let mut no_art = atlas;
        no_art
            .sprites
            .remove(sprite(Button::LeftTrigger, GlyphStyle::Nintendo).unwrap());
        let label = HudBatch::text_width(&no_art, "ZL").ceil() + 4.0;
        assert_eq!(
            width(&no_art, Button::LeftTrigger, GlyphStyle::Nintendo),
            label
        );
    }

    #[test]
    fn an_atlas_without_the_art_falls_back_to_text() {
        let mut atlas = atlas();
        atlas
            .sprites
            .remove(sprite(Button::South, GlyphStyle::Xbox).unwrap());
        assert_eq!(art(&atlas, Button::South, GlyphStyle::Xbox), None);
        let label = HudBatch::text_width(&atlas, "A").ceil() + 4.0;
        assert_eq!(width(&atlas, Button::South, GlyphStyle::Xbox), label);
    }

    #[test]
    fn art_is_one_quad_centred_on_the_text_line_with_no_key_behind_it() {
        let atlas = atlas();
        let mut batch = HudBatch::new([640, 480]);
        draw(
            &mut batch,
            &atlas,
            Button::South,
            GlyphStyle::Xbox,
            10.0,
            20.0,
        );
        assert_eq!(quads(&batch), vec![[7.0, 16.0, 23.0, 32.0]]);
        let mut batch = HudBatch::new([640, 480]);
        draw(
            &mut batch,
            &atlas,
            Button::LeftBumper,
            GlyphStyle::Xbox,
            10.0,
            20.0,
        );
        assert_eq!(quads(&batch), vec![[7.0, 16.0, 31.0, 32.0]]);
    }

    #[test]
    fn text_sits_on_a_key_covering_what_rect_says() {
        let mut atlas = atlas();
        let style = GlyphStyle::Nintendo;
        atlas.sprites.remove(sprite(Button::South, style).unwrap());
        let mut batch = HudBatch::new([640, 480]);
        draw(&mut batch, &atlas, Button::South, style, 10.0, 20.0);
        let [x, y, w, h] = rect(&atlas, Button::South, style, 10.0, 20.0);
        assert_eq!(quads(&batch)[0], [x, y, x + w, y + h]);
    }
}
