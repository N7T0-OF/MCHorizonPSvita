//! Horizon saved world-generation options and preset identifiers.

use crate::{
    level_dat::world_gen_settings,
    nbt::Tag,
    nbt_value::{number, text},
};
use std::path::Path;

/// The world presets of 26.3's `worldgen/world_preset` that the create
/// screen offers, in the order of `WorldPresetTags.NORMAL`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldPreset {
    Normal,
    Flat,
    LargeBiomes,
    Amplified,
    SingleBiomeSurface,
    /// `minecraft:debug_all_block_states`: a grid of every block state
    /// (`DebugLevelSource`). The create screen does not offer it; a data
    /// pack, `level-type` or a saved world names it.
    Debug,
}
impl WorldPreset {
    /// The presets, in the order the world-type button cycles through them.
    pub const ALL: [Self; 5] = [
        Self::Normal,
        Self::Flat,
        Self::LargeBiomes,
        Self::Amplified,
        Self::SingleBiomeSurface,
    ];
    /// The preset's registry id (`minecraft:normal`).
    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "minecraft:normal",
            Self::Flat => "minecraft:flat",
            Self::LargeBiomes => "minecraft:large_biomes",
            Self::Amplified => "minecraft:amplified",
            Self::SingleBiomeSurface => "minecraft:single_biome_surface",
            Self::Debug => "minecraft:debug_all_block_states",
        }
    }
    /// The preset an id names, if it is one of them.
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .chain([Self::Debug])
            .find(|preset| preset.id() == id)
    }
    /// The language key the world-type button shows
    /// (`Identifier.toLanguageKey("generator")`).
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "generator.minecraft.normal",
            Self::Flat => "generator.minecraft.flat",
            Self::LargeBiomes => "generator.minecraft.large_biomes",
            Self::Amplified => "generator.minecraft.amplified",
            Self::SingleBiomeSurface => "generator.minecraft.single_biome_surface",
            Self::Debug => "generator.minecraft.debug_all_block_states",
        }
    }
    /// The preset the world-type button cycles to next.
    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|preset| *preset == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
    /// The preset the world-type button cycles to previously.
    pub fn prev(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|preset| *preset == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
    /// The preset a saved overworld generator is: the noise settings name
    /// the preset, and the single-biome preset shares the overworld's noise
    /// settings, so its fixed biome source tells the two apart.
    pub fn of_generator(generator: &Tag) -> Self {
        if text(generator.get("type")).as_deref() == Some("minecraft:flat") {
            return Self::Flat;
        }
        if text(generator.get("type")).as_deref() == Some("minecraft:debug") {
            return Self::Debug;
        }
        let source = generator.get("biome_source");
        let fixed = text(source.and_then(|s| s.get("type"))).as_deref() == Some("minecraft:fixed");
        match text(generator.get("settings")).as_deref() {
            Some("minecraft:large_biomes") => Self::LargeBiomes,
            Some("minecraft:amplified") => Self::Amplified,
            Some("minecraft:overworld") if fixed => Self::SingleBiomeSurface,
            _ => Self::Normal,
        }
    }
}
/// World and player state stored in `level.dat`, the saved-data files and
/// the player file.
/// The world options CreateWorldScreen sets (`WorldOptions` and the world
/// preset) that `world_gen_settings` keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldGen {
    /// The world preset the world is created with.
    pub preset: WorldPreset,
    pub structures: bool,
    pub bonus_chest: bool,
}
impl WorldGen {
    /// A saved world's choices, from its `world_gen_settings`.
    pub fn read(world: &Path) -> Option<Self> {
        let settings = world_gen_settings(world)?;
        let flag = |key: &str, default: bool| number(settings.get(key)).map_or(default, |v| v != 0);
        let generator = settings
            .get("dimensions")
            .and_then(|d| d.get("minecraft:overworld"))
            .and_then(|o| o.get("generator"));
        Some(Self {
            preset: generator.map_or(WorldPreset::Normal, WorldPreset::of_generator),
            structures: flag("generate_structures", true),
            bonus_chest: flag("bonus_chest", false),
        })
    }
    /// The bundled world preset a saved world's overworld was generated
    /// with, from its generator: `minecraft:amplified` and
    /// `minecraft:large_biomes` by their noise settings, and
    /// `minecraft:single_biome_surface` by a fixed biome source. Anything
    /// else (and any world without settings) is `minecraft:normal`.
    pub fn preset(world: &Path) -> &'static str {
        let settings = world_gen_settings(world);
        let generator = settings
            .as_ref()
            .and_then(|s| s.get("dimensions"))
            .and_then(|d| d.get("minecraft:overworld"))
            .and_then(|o| o.get("generator"));
        let noise = text(generator.and_then(|g| g.get("settings")));
        let fixed_biome = generator
            .and_then(|g| g.get("biome_source"))
            .and_then(|b| text(b.get("type")))
            .is_some_and(|t| t == "minecraft:fixed");
        match noise.as_deref() {
            Some("minecraft:amplified") => "minecraft:amplified",
            Some("minecraft:large_biomes") => "minecraft:large_biomes",
            Some("minecraft:overworld") if fixed_biome => "minecraft:single_biome_surface",
            _ => "minecraft:normal",
        }
    }
}
impl Default for WorldGen {
    fn default() -> Self {
        Self {
            preset: WorldPreset::Normal,
            structures: true,
            bonus_chest: false,
        }
    }
}
