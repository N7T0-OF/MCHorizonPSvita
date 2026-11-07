//! Horizon data-pack discovery shared by its screens and renderer.

use horizon_resource_zip::Zip;
use std::path::{Path, PathBuf};

/// A pack on disk.
pub struct DataPack {
    /// `file/<name>` in `/datapack` and level.dat.
    pub id: String,
    source: Source,
}
enum Source {
    Dir(PathBuf),
    Zip(Box<Zip>),
}
impl DataPack {
    /// Every file path under `prefix` (relative to the pack root).
    pub fn list(&self, prefix: &str) -> Vec<String> {
        match &self.source {
            Source::Zip(z) => z
                .names()
                .filter(|n| n.starts_with(prefix) && !n.ends_with('/'))
                .map(str::to_owned)
                .collect(),
            Source::Dir(root) => {
                let mut out = Vec::new();
                walk(root, &root.join(prefix), &mut out);
                out
            }
        }
    }
    /// The files of `data/<namespace>/<kind>/` for each kind, in the order
    /// `list("data/")` gives them. A folder pack walks only those folders:
    /// the built-in pack holds about 10,000 other data files.
    #[doc(hidden)]
    pub fn list_kinds(&self, kinds: &[&str]) -> Vec<String> {
        debug_assert!(kinds.is_sorted(), "kinds in walk order");
        let Source::Dir(root) = &self.source else {
            return self.list("data/");
        };
        let Ok(entries) = std::fs::read_dir(root.join("data")) else {
            return Vec::new();
        };
        let mut namespaces: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        namespaces.sort();
        let mut out = Vec::new();
        for ns in namespaces {
            for kind in kinds {
                walk(root, &ns.join(kind), &mut out);
            }
        }
        out
    }
    /// The files of `data/<namespace>/<folder>/` in every namespace, as
    /// `list` names them; a folder pack walks only those folders.
    pub fn list_folder(&self, folder: &str) -> Vec<String> {
        let files = self.list_kinds(&[folder]);
        let Source::Zip(_) = &self.source else {
            return files;
        };
        files
            .into_iter()
            .filter(|f| {
                f.strip_prefix("data/")
                    .and_then(|r| r.split_once('/'))
                    .is_some_and(|(_, r)| {
                        r.strip_prefix(folder).is_some_and(|r| r.starts_with('/'))
                    })
            })
            .collect()
    }
    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        match &self.source {
            Source::Zip(z) => z.read(path).ok(),
            Source::Dir(root) => std::fs::read(root.join(path)).ok(),
        }
    }

    /// The `description` of `pack.mcmeta`, as text.
    pub fn description(&self) -> String {
        let Some(bytes) = self.read("pack.mcmeta") else {
            return String::new();
        };
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        match &v["pack"]["description"] {
            serde_json::Value::String(s) => s.clone(),
            other => other["text"].as_str().unwrap_or_default().to_owned(),
        }
    }
    /// The `features.enabled` flag ids of `pack.mcmeta`
    /// (`FeatureFlagsMetadataSection`): the flags the pack asks to turn on.
    pub fn features(&self) -> Vec<String> {
        self.mcmeta()["features"]["enabled"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|f| f.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }
    /// The `pack.mcmeta` as JSON, empty when it is missing or unreadable.
    #[doc(hidden)]
    pub fn mcmeta(&self) -> serde_json::Value {
        self.read("pack.mcmeta")
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
}
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            walk(root, &p, out);
        } else if let Ok(rel) = p.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}
impl DataPack {
    /// The built-in `vanilla` pack: the game's own data directory.
    pub fn builtin(root: &Path) -> Self {
        DataPack {
            id: "vanilla".into(),
            source: Source::Dir(root.to_path_buf()),
        }
    }
}
/// The packs in `<world>/datapacks`, sorted by name.
pub fn discover(world: &Path) -> Vec<DataPack> {
    discover_folder(&world.join("datapacks"))
}
/// Packs in Create World's temporary pack folder, before a world exists.
pub fn discover_folder(dir: &Path) -> Vec<DataPack> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut packs = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let path = e.path();
        if path.is_dir() && path.join("pack.mcmeta").exists() {
            packs.push(DataPack {
                id: format!("file/{name}"),
                source: Source::Dir(path),
            });
        } else if name.ends_with(".zip")
            && let Ok(z) = Zip::open(&path)
            && z.contains("pack.mcmeta")
        {
            packs.push(DataPack {
                id: format!("file/{name}"),
                source: Source::Zip(Box::new(z)),
            });
        }
    }
    packs.sort_by(|a, b| a.id.cmp(&b.id));
    packs
}
/// The resources root's built-in feature packs (`ServerPacksSource`
/// scanning the game's `data/minecraft/datapacks`): every folder with a
/// `pack.mcmeta`, a feature pack (`PackSource.FEATURE`). Off until a world
/// selects them.
pub fn feature_packs(resources: &Path) -> Vec<DataPack> {
    let Ok(entries) = std::fs::read_dir(resources.join("data").join("minecraft").join("datapacks"))
    else {
        return Vec::new();
    };
    let mut packs = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        if path.join("pack.mcmeta").exists() {
            packs.push(DataPack {
                id: e.file_name().to_string_lossy().into_owned(),
                source: Source::Dir(path),
            });
        }
    }
    packs.sort_by(|a, b| a.id.cmp(&b.id));
    packs
}
/// The packs of `<world>/datapacks` plus the resources root's feature packs:
/// the world's whole repository, as `PackRepository.reload` sees it.
pub fn discover_with_features(world_root: &Path, resources: &Path) -> Vec<DataPack> {
    let mut packs = discover(world_root);
    packs.extend(feature_packs(resources));
    packs
}
