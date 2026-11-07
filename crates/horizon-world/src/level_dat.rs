//! Horizon's level.dat reading, writing and recovery diagnosis.
use crate::{
    compression::Compression,
    nbt::{self, Limits, NamedTag, Tag},
};
use std::{
    fs,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

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

/// `LevelStorageSource.readLevelDataTagFixed`: level.dat, else level.dat_old.
#[doc(hidden)]
pub fn read_level_dat(dir: &Path) -> Option<Tag> {
    read_dat(&dir.join("level.dat")).or_else(|| read_dat(&dir.join("level.dat_old")))
}

/// Write a gzip NBT file the way vanilla does (`<name>_new`, keep `_old`).
pub fn write_dat(path: &Path, tag: Tag) -> Result<()> {
    write_dat_as(path, tag, false)
}

/// [`write_dat`], synced to the disk: the new file before it is renamed into place, and the
/// folder after, so a power loss leaves the old file or the new one. A hand-over's commit
/// point (`player-confirm`) replies only once its files are.
pub fn write_dat_durable(path: &Path, tag: Tag) -> Result<()> {
    write_dat_as(path, tag, true)
}

/// `bytes` written to `path` through `<name>_new` and a rename, synced to the disk as
/// [`write_dat_durable`] syncs: never a half-written file in place.
pub fn write_durable(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("data");
    let temporary = path.with_file_name(format!("{file}_new"));
    write_synced(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    sync_folder(path)?;
    Ok(())
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// The folder of `path` synced, so that a rename into it is on the disk (POSIX; Windows has
/// no folder handle to sync, and its rename is journaled by NTFS).
fn sync_folder(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    if let Some(dir) = path.parent() {
        fs::File::open(dir)?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn write_dat_as(path: &Path, tag: Tag, durable: bool) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let bytes = nbt::write_named(
        &NamedTag {
            name: "".into(),
            tag,
        },
        Limits::saved(),
    )
    .map_err(|e| e.to_string())?;
    let compressed = Compression::Gzip.encode(&bytes)?;
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("data.dat");
    let temporary = path.with_file_name(format!("{file}_new"));
    if durable {
        write_synced(&temporary, &compressed)?;
    } else {
        fs::write(&temporary, compressed)?;
    }
    if path.exists() {
        let old = match file.strip_suffix(".dat") {
            Some(stem) => path.with_file_name(format!("{stem}.dat_old")),
            None => path.with_file_name(format!("{file}_old")),
        };
        // Util.safeReplaceFile: no backup, no replacement.
        fs::rename(path, old).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    fs::rename(temporary, path)?;
    if durable {
        sync_folder(path)?;
    }
    Ok(())
}

#[doc(hidden)]
pub fn write_saved_data(world: &Path, name: &str, data: Tag) -> Result<()> {
    write_dat(
        &saved_data_path(world, name),
        Tag::compound([
            ("data", data),
            ("DataVersion", Tag::Int(crate::DATA_VERSION)),
        ]),
    )
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
        assert_eq!(read_dat(&dir.join("level.dat")), Some(root.clone()));
        fs::remove_file(dir.join("level.dat_old")).unwrap();
        let next = Tag::compound([(
            "Data",
            Tag::compound([("LevelName", Tag::String("Next".into()))]),
        )]);
        write_dat(&dir.join("level.dat"), next.clone()).unwrap();
        assert_eq!(read_dat(&dir.join("level.dat")), Some(next));
        assert_eq!(read_dat(&dir.join("level.dat_old")), Some(root));
        fs::remove_dir_all(dir).unwrap();
    }
}
