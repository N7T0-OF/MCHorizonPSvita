//! Horizon's PNG reader shared by font loading and the HUD atlas.
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use horizon_client_core::Result;

/// `TextureMetadataSection`'s `blur` and `clamp`: how vanilla samples a
/// standalone texture (`ReloadableTexture.setSampler`: linear rather than
/// nearest filtering, clamp-to-edge rather than repeat addressing).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureFlags {
    pub blur: bool,
    pub clamp: bool,
}
impl TextureFlags {
    /// The `texture` section of a `.png.mcmeta` file, if it sets either flag.
    pub fn from_mcmeta(json: &str) -> Option<Self> {
        let meta: serde_json::Value = serde_json::from_str(json).ok()?;
        let section = meta.get("texture")?;
        let flag = |key| section.get(key).and_then(serde_json::Value::as_bool) == Some(true);
        let flags = Self {
            blur: flag("blur"),
            clamp: flag("clamp"),
        };
        (flags != Self::default()).then_some(flags)
    }
}

pub fn decode(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    decode_bytes(&horizon_core::prepared_files::read(path)?)
}

/// A PNG in memory as RGBA pixels.
pub fn decode_bytes(data: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.ignore_checksums(true);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let mut bytes = vec![0; reader.output_buffer_size().ok_or("image too large")?];
    let info = reader.next_frame(&mut bytes)?;
    let channels = info.color_type.samples();
    let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
    for p in bytes[..info.buffer_size()].chunks_exact(channels) {
        rgba.extend(match channels {
            4 => [p[0], p[1], p[2], p[3]],
            3 => [p[0], p[1], p[2], 255],
            2 => [p[0], p[0], p[0], p[1]],
            _ => [p[0], p[0], p[0], 255],
        });
    }
    Ok((info.width, info.height, rgba))
}

/// The variant images (and `overlays`) the atlas packs: every variant before any overlay, by
/// name, while their sizes add up to at most `budget` texels. A full budget drops glows before
/// looks.
pub fn budgeted<'a>(
    files: impl Iterator<Item = &'a PathBuf>,
    overlays: &HashSet<PathBuf>,
    budget: u64,
) -> HashSet<PathBuf> {
    let mut files: Vec<&PathBuf> = files.collect();
    files.sort_by_key(|p| (overlays.contains(*p), *p));
    within_budget(&files, budget)
}
/// The `files` (in order) whose sizes add up to at most `budget` texels.
pub fn within_budget(files: &[&PathBuf], budget: u64) -> HashSet<PathBuf> {
    let mut texels = 0u64;
    files
        .iter()
        .filter(|p| {
            let (w, h) = png_size(p).unwrap_or((0, 0));
            texels += u64::from(w) * u64::from(h);
            texels <= budget
        })
        .map(|p| (*p).clone())
        .collect()
}
/// A PNG's width and height from its first 24 bytes (`IHDR`), before any
/// pixel buffer is made.
pub fn png_bytes_size(data: &[u8]) -> Option<(u32, u32)> {
    let header = data.get(..24)?;
    if header[..8] != *b"\x89PNG\r\n\x1a\n" || header[12..16] != *b"IHDR" {
        return None;
    }
    let field = |at: usize| u32::from_be_bytes(header[at..at + 4].try_into().unwrap_or_default());
    Some((field(16), field(20)))
}
/// A PNG's width and height, from its header alone.
pub fn png_size(path: &Path) -> Option<(u32, u32)> {
    png_bytes_size(&horizon_core::prepared_files::read_head(path, 24).ok()?)
}
pub fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    // A prepared root's folder: from its data file, without listing the disk.
    if let Some(files) = horizon_core::prepared_files::files_under(dir) {
        out.extend(
            files
                .iter()
                .filter(|file| file.ends_with(".png"))
                .map(|file| dir.join(file)),
        );
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "png") {
            out.push(path);
        }
    }
}
