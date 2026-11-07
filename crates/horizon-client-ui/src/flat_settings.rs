//! Horizon Superflat settings and their original save representation.

use horizon_world::nbt::Tag;
use std::{collections::BTreeSet, path::Path};

/// DimensionType.Y_SIZE: the most blocks a flat generator takes.
const Y_SIZE: i32 = 4064;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatLayer {
    /// A block id such as `minecraft:dirt`.
    pub block: String,
    /// How many blocks tall this layer is (0..=4064).
    pub height: i32,
}

/// FlatLevelGeneratorSettings: the layers (bottom to top), the biome and the
/// decorations CreateFlatWorldScreen edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatSettings {
    pub biome: String,
    pub features: bool,
    pub lakes: bool,
    pub layers: Vec<FlatLayer>,
    pub structure_overrides: Vec<String>,
}
impl Default for FlatSettings {
    fn default() -> Self {
        // worldgen/world_preset/flat.json's overworld.
        Self {
            biome: "minecraft:plains".into(),
            features: false,
            lakes: false,
            layers: vec![
                FlatLayer {
                    block: "minecraft:bedrock".into(),
                    height: 1,
                },
                FlatLayer {
                    block: "minecraft:dirt".into(),
                    height: 2,
                },
                FlatLayer {
                    block: "minecraft:grass_block".into(),
                    height: 1,
                },
            ],
            structure_overrides: vec!["minecraft:strongholds".into(), "minecraft:villages".into()],
        }
    }
}
impl FlatSettings {
    /// A saved world's flat generator settings, for Re-Create.
    pub fn from_world(dir: &Path) -> Option<Self> {
        let settings = horizon_world::level_dat::world_gen_settings(dir)?;
        let generator = settings
            .get("dimensions")?
            .get("minecraft:overworld")?
            .get("generator")?;
        if horizon_world::nbt_value::text(generator.get("type"))?.trim_start_matches("minecraft:")
            != "flat"
        {
            return None;
        }
        Self::from_tag(generator.get("settings")?)
    }
    pub fn from_tag(tag: &Tag) -> Option<Self> {
        let Some(Tag::List(list)) = tag.get("layers") else {
            return None;
        };
        let layers: Vec<FlatLayer> = list
            .iter()
            .filter_map(|layer| {
                Some(FlatLayer {
                    block: horizon_world::nbt_value::text(layer.get("block"))?,
                    height: horizon_world::nbt_value::number(layer.get("height"))?
                        .clamp(0, Y_SIZE as i64) as i32,
                })
            })
            .collect();
        // `layers: []` is a valid void flat world; a list whose entries all
        // fail to parse is not.
        if layers.is_empty() && !list.is_empty() {
            return None;
        }
        let flag = |key: &str, default: bool| {
            horizon_world::nbt_value::number(tag.get(key)).map_or(default, |value| value != 0)
        };
        let structure_overrides = match tag.get("structure_overrides") {
            Some(Tag::String(text)) => vec![text.to_string_lossy()],
            Some(Tag::List(list)) => list
                .iter()
                .filter_map(|value| horizon_world::nbt_value::text(Some(value)))
                .collect(),
            _ => Vec::new(),
        };
        Some(Self {
            biome: horizon_world::nbt_value::text(tag.get("biome"))
                .unwrap_or_else(|| "minecraft:plains".into()),
            features: flag("features", false),
            lakes: flag("lakes", false),
            layers,
            structure_overrides,
        })
    }
    /// FlatLevelGeneratorSettings.CODEC, as world_gen_settings stores it.
    pub fn to_tag(&self) -> Tag {
        let string = |text: &str| Tag::String(text.into());
        Tag::compound([
            ("biome", string(&self.biome)),
            ("features", Tag::Byte(i8::from(self.features))),
            ("lakes", Tag::Byte(i8::from(self.lakes))),
            (
                "layers",
                Tag::List(
                    self.layers
                        .iter()
                        .map(|layer| {
                            Tag::compound([
                                ("block", string(&layer.block)),
                                ("height", Tag::Int(layer.height)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "structure_overrides",
                Tag::List(
                    self.structure_overrides
                        .iter()
                        .map(|id| string(id))
                        .collect(),
                ),
            ),
        ])
    }
    /// `CreateFlatWorldScreen.save`: the text the preset screen shows.
    pub fn to_share(&self) -> String {
        let layers = self
            .layers
            .iter()
            .map(|layer| {
                if layer.height == 1 {
                    layer.block.clone()
                } else {
                    format!("{}*{}", layer.height, layer.block)
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("{layers};{}", self.biome)
    }
    /// `PresetFlatWorldScreen.fromString`: `layers;biome`, keeping the
    /// decorations the screen was opened with. `None` for a malformed list,
    /// as vanilla falls back to the preset's own settings.
    pub fn parse(
        text: &str,
        blocks: &BTreeSet<String>,
        biomes: &BTreeSet<String>,
        base: &Self,
    ) -> Option<Self> {
        let (layer_text, biome_text) = text.split_once(';').unwrap_or((text, ""));
        let mut layers = Vec::new();
        let mut total = 0;
        for part in layer_text.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (height, block) = match part.split_once('*') {
                Some((height, block)) => (height.trim().parse::<i32>().ok()?, block.trim()),
                None => (1, part),
            };
            let block = namespaced(block);
            if !blocks.is_empty() && !blocks.contains(&block) {
                return None;
            }
            let height = height.max(0).min(Y_SIZE - total);
            if height == 0 {
                continue;
            }
            total += height;
            layers.push(FlatLayer { block, height });
        }
        if layers.is_empty() {
            return None;
        }
        let mut biome = namespaced(biome_text.trim());
        if biome == "minecraft:" || biome.is_empty() {
            biome = "minecraft:plains".into();
        }
        if !biomes.is_empty() && !biomes.contains(&biome) {
            biome = "minecraft:plains".into();
        }
        Some(Self {
            biome,
            layers,
            ..base.clone()
        })
    }
}
/// `Identifier.parse`: bare names get the default namespace.
fn namespaced(id: &str) -> String {
    if id.is_empty() || id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_string_and_nbt_keep_the_requested_layers() {
        let blocks = ["minecraft:stone".to_owned(), "minecraft:dirt".to_owned()]
            .into_iter()
            .collect();
        let biomes = ["minecraft:desert".to_owned()].into_iter().collect();
        let settings = FlatSettings::parse(
            "2*stone,dirt;minecraft:desert",
            &blocks,
            &biomes,
            &FlatSettings::default(),
        )
        .unwrap();
        let expected = Tag::compound([
            ("biome", Tag::String("minecraft:desert".into())),
            ("features", Tag::Byte(0)),
            ("lakes", Tag::Byte(0)),
            (
                "layers",
                Tag::List(vec![
                    Tag::compound([
                        ("block", Tag::String("minecraft:stone".into())),
                        ("height", Tag::Int(2)),
                    ]),
                    Tag::compound([
                        ("block", Tag::String("minecraft:dirt".into())),
                        ("height", Tag::Int(1)),
                    ]),
                ]),
            ),
            (
                "structure_overrides",
                Tag::List(vec![
                    Tag::String("minecraft:strongholds".into()),
                    Tag::String("minecraft:villages".into()),
                ]),
            ),
        ]);
        assert_eq!(settings.to_tag(), expected);
        assert_eq!(FlatSettings::from_tag(&expected), Some(settings));
        assert!(
            FlatSettings::parse(
                "minecraft:unknown;minecraft:desert",
                &blocks,
                &biomes,
                &FlatSettings::default(),
            )
            .is_none()
        );
    }
}
