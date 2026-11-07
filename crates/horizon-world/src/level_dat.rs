//! Horizon's level.dat reading and recovery diagnosis.
use crate::{
    compression::Compression,
    nbt::{self, Limits, Tag},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn read_dat(path: &Path) -> Option<Tag> {
    try_read_dat(path).ok()
}
/// `read_dat`, with why the file did not read.
#[doc(hidden)]
pub fn try_read_dat(path: &Path) -> std::result::Result<Tag, DatIssue> {
    let bytes = fs::read(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => DatIssue::Missing,
        _ => DatIssue::Unreadable(e.to_string()),
    })?;
    let raw = match Compression::Gzip.decode(&bytes, Limits::saved().max_bytes) {
        Ok(raw) => raw,
        Err(_) => bytes,
    };
    nbt::read_named(&raw, Limits::saved())
        .map(|(named, _)| named.tag)
        .map_err(|e| DatIssue::Unreadable(e.to_string()))
}
/// What `LevelStorageAccess.collectIssues` finds wrong with a level.dat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DatIssue {
    /// No such file (`FileNotFoundException`).
    Missing,
    /// It exists but does not read as a level: why.
    Unreadable(String),
}
/// `LevelStorageAccess.collectIssues`: why level.dat (level.dat_old with
/// `old`) can't be read as the world's level data, `None` when it can.
pub fn level_dat_issue(dir: &Path, old: bool) -> Option<DatIssue> {
    let file = if old { "level.dat_old" } else { "level.dat" };
    match try_read_dat(&dir.join(file)) {
        Ok(root) if matches!(root.get("Data"), Some(Tag::Compound(_))) => None,
        Ok(_) => Some(DatIssue::Unreadable("Invalid root tag".into())),
        Err(issue) => Some(issue),
    }
}
/// `WorldOpenFlows.openWorldLoadLevelData`: what keeps `world`'s level data from reading, which
/// opens RecoverWorldDataScreen instead of the world. A folder with neither level.dat nor
/// level.dat_old is a world not made yet, not a broken one.
pub fn level_data_needs_recovery(world: &Path) -> Option<DatIssue> {
    match level_dat_issue(world, false)? {
        DatIssue::Missing if !world.join("level.dat_old").exists() => None,
        issue => Some(issue),
    }
}

/// `SavedDataStorage`: `{data: ..., DataVersion}` under `data/minecraft/`.
#[doc(hidden)]
pub fn saved_data_path(world: &Path, name: &str) -> PathBuf {
    world
        .join("data")
        .join("minecraft")
        .join(format!("{name}.dat"))
}
#[doc(hidden)]
pub fn read_saved_data(world: &Path, name: &str) -> Option<Tag> {
    read_dat(&saved_data_path(world, name))?
        .get("data")
        .cloned()
}
/// The world's generator settings: world_gen_settings.dat (26.3) or
/// level.dat's Data.WorldGenSettings.
pub fn world_gen_settings(world: &Path) -> Option<Tag> {
    // A world still to be file-fixed has them in level.dat in an older shape: they are read as
    // the fixes would leave them, so the questions on opening it are the same ones.
    if let Ok(Some(settings)) = crate::datafix::files::current_world_gen_settings(world) {
        return Some(settings);
    }
    read_saved_data(world, "world_gen_settings").or_else(|| {
        read_dat(&world.join("level.dat"))?
            .get("Data")?
            .get("WorldGenSettings")
            .cloned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nbt::NamedTag;

    #[test]
    fn diagnoses_missing_corrupt_and_valid_level_data() {
        let dir = std::env::temp_dir().join(format!(
            "horizon-level-dat-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        assert_eq!(level_data_needs_recovery(&dir), None);
        fs::write(dir.join("level.dat_old"), b"broken").unwrap();
        assert_eq!(level_data_needs_recovery(&dir), Some(DatIssue::Missing));
        fs::write(dir.join("level.dat"), b"broken").unwrap();
        assert!(matches!(
            level_data_needs_recovery(&dir),
            Some(DatIssue::Unreadable(_))
        ));

        let root = Tag::compound([(
            "Data",
            Tag::compound([("LevelName", Tag::String("World".into()))]),
        )]);
        let raw = nbt::write_named(
            &NamedTag {
                name: "".into(),
                tag: root.clone(),
            },
            Limits::saved(),
        )
        .unwrap();
        let compressed = Compression::Gzip.encode(&raw).unwrap();
        fs::write(dir.join("level.dat"), compressed).unwrap();
        assert_eq!(read_dat(&dir.join("level.dat")), Some(root.clone()));
        assert_eq!(level_dat_issue(&dir, false), None);
        fs::write(dir.join("level.dat"), raw).unwrap();
        assert_eq!(read_dat(&dir.join("level.dat")), Some(root));
        fs::remove_dir_all(dir).unwrap();
    }
}
