//! Horizon controller button artwork and its CPU decoding.
use crate::pad_cursor;
use horizon_client_input::gamepad::{Button, GlyphStyle};

/// The art's height as drawn, in GUI pixels. Its texture is twice that, so it maps pixel for pixel
/// at GUI scale 2 and to whole pixels at 4.
const DRAWN_HEIGHT: f32 = 16.0;
/// The clear edge the art has all round.
pub const MARGIN: f32 = 2.0;
/// How tall a glyph shows, art or key: the text line and 2 above and below it.
pub const HEIGHT: f32 = DRAWN_HEIGHT - 2.0 * MARGIN;
/// How wide a face button's art shows; a declared button's label leaves it room.
pub const FACE_WIDTH: f32 = 12.0;

/// A picture of a button in a family (Auto reads as Xbox, as `Button::label` has it). Every family
/// has its own file for every button, some of them a copy of another's until theirs is drawn.
pub struct Art {
    family: GlyphStyle,
    button: Button,
    /// The atlas sprite: where a pack's own file lands.
    pub sprite: &'static str,
    png: &'static [u8],
}

macro_rules! art {
    ($($family:ident $button:ident $file:literal,)*) => {
        &[$(Art {
            family: GlyphStyle::$family,
            button: Button::$button,
            sprite: concat!("horizon:gui/controller/", $file),
            png: include_bytes!(concat!(
                "../../horizon-client/assets/horizon/textures/gui/controller/",
                $file,
                ".png"
            )),
        },)*]
    };
}

/// All the art. A new picture is one line here and its file: a button not listed is text, and a
/// picture is measured from its texture, so it can be as wide as it needs.
static ART: &[Art] = art![
    Xbox South "xb_a",
    Xbox East "xb_b",
    Xbox West "xb_x",
    Xbox North "xb_y",
    Xbox LeftBumper "xb_lb",
    Xbox RightBumper "xb_rb",
    Xbox LeftTrigger "xb_lt",
    Xbox RightTrigger "xb_rt",
    Xbox Select "xb_view",
    Xbox Start "xb_menu",
    Xbox LeftStick "xb_l3",
    Xbox RightStick "xb_r3",
    Xbox DPadUp "xb_up",
    Xbox DPadDown "xb_down",
    Xbox DPadLeft "xb_left",
    Xbox DPadRight "xb_right",
    PlayStation South "psx_cross",
    PlayStation East "psx_circle",
    PlayStation West "psx_square",
    PlayStation North "psx_triangle",
    PlayStation LeftBumper "psx_l1",
    PlayStation RightBumper "psx_r1",
    PlayStation LeftTrigger "psx_l2",
    PlayStation RightTrigger "psx_r2",
    PlayStation Select "psx_create",
    PlayStation Start "psx_options",
    PlayStation LeftStick "psx_l3",
    PlayStation RightStick "psx_r3",
    PlayStation DPadUp "psx_up",
    PlayStation DPadDown "psx_down",
    PlayStation DPadLeft "psx_left",
    PlayStation DPadRight "psx_right",
    Nintendo South "nx_b",
    Nintendo East "nx_a",
    Nintendo West "nx_y",
    Nintendo North "nx_x",
    Nintendo LeftBumper "nx_l",
    Nintendo RightBumper "nx_r",
    Nintendo LeftTrigger "nx_zl",
    Nintendo RightTrigger "nx_zr",
    Nintendo Select "nx_minus",
    Nintendo Start "nx_plus",
    Nintendo LeftStick "nx_l3",
    Nintendo RightStick "nx_r3",
    Nintendo DPadUp "nx_up",
    Nintendo DPadDown "nx_down",
    Nintendo DPadLeft "nx_left",
    Nintendo DPadRight "nx_right",
];

pub fn find(button: Button, style: GlyphStyle) -> Option<&'static Art> {
    let family = match style {
        GlyphStyle::Auto => GlyphStyle::Xbox,
        style => style,
    };
    ART.iter()
        .find(|art| art.family == family && art.button == button)
}

/// The sprite of a button in a family, if it has one.
pub fn sprite(button: Button, style: GlyphStyle) -> Option<&'static str> {
    find(button, style).map(|art| art.sprite)
}

/// How wide the art shows: half its texture (IHDR's width, in a PNG's first bytes) less its edge.
/// A pack's own texture of another size is drawn in the same place.
pub fn art_width(art: &Art) -> f32 {
    let texels = u32::from_be_bytes([art.png[16], art.png[17], art.png[18], art.png[19]]);
    texels as f32 / 2.0 - 2.0 * MARGIN
}

/// Horizon's art for the atlas: each `(sprite, width, height, RGBA texels)`.
pub fn images() -> impl Iterator<Item = (String, u32, u32, Vec<u8>)> {
    ART.iter()
        .map(|art| (art.sprite, art.png))
        .chain([(pad_cursor::SPRITE, pad_cursor::PNG)])
        .map(|(name, png)| {
            let (w, h, pixels) = decode(png).expect("Horizon's controller art is PNG");
            (name.to_owned(), w, h, pixels)
        })
}

fn decode(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let (w, h) = (reader.info().width, reader.info().height);
    let mut buf = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    let channels = frame.color_type.samples();
    let pixels = buf
        .chunks(channels)
        .take((w * h) as usize)
        .flat_map(|p| match channels {
            4 => [p[0], p[1], p[2], p[3]],
            3 => [p[0], p[1], p[2], 255],
            2 => [p[0], p[0], p[0], p[1]],
            _ => [p[0], p[0], p[0], 255],
        })
        .collect();
    Some((w, h, pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sprite_has_its_texture_at_twice_gui_size() {
        let images: Vec<_> = images().collect();
        assert_eq!(images.len(), ART.len() + 1, "the art and the cursor");
        for (name, w, h, pixels) in &images {
            assert_eq!(pixels.len(), (w * h * 4) as usize, "{name}");
            assert_eq!(*h as f32, DRAWN_HEIGHT * 2.0, "{name}");
        }
        for art in ART {
            let (_, w, ..) = images.iter().find(|i| i.0 == art.sprite).unwrap();
            assert_eq!(art_width(art), *w as f32 / 2.0 - 4.0, "{}", art.sprite);
        }
    }

    #[test]
    fn no_button_has_two_pictures_in_a_family() {
        for (i, art) in ART.iter().enumerate() {
            assert!(
                !ART[..i]
                    .iter()
                    .any(|a| a.family == art.family && a.button == art.button),
                "{}",
                art.sprite
            );
        }
    }
}
