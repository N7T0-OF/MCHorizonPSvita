/// What the section mesher does with leaf faces inside a canopy. Terrain
/// geometry only: entities, particles and block entities are untouched.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LeafCullingMode {
    /// Every leaf face, as vanilla's fancy leaves draw them.
    #[default]
    Off,
    /// Drops a face between two leaves when the leaf it faces has a leaf or
    /// a full opaque cube behind it too: that face only shows through two
    /// cutout layers. The faces behind the outer layer stay, so a crown
    /// looks the same from outside. The faced leaf must be in the section
    /// (as `Hollow`), so the block behind it is at most a border cell of the
    /// next section, whose edits remesh this one.
    Hidden,
    /// Leaves surrounded on all six sides by leaves or full opaque cubes draw
    /// in the opaque layer, so their cutouts no longer show the layers behind.
    Solid,
    /// Like `Solid`, but a leaf with one open side draws opaque too: the
    /// exposed shell thins by a layer and the canopy looks chunkier.
    SolidAggressive,
    /// Drops the faces two leaves share. Faces toward air, cavities, partial
    /// models, unknown neighbours and section boundaries stay.
    Hollow,
}

impl LeafCullingMode {
    pub const ALL: [LeafCullingMode; 5] = [
        LeafCullingMode::Off,
        LeafCullingMode::Hidden,
        LeafCullingMode::Solid,
        LeafCullingMode::SolidAggressive,
        LeafCullingMode::Hollow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LeafCullingMode::Off => "off",
            LeafCullingMode::Hidden => "hidden",
            LeafCullingMode::Solid => "solid",
            LeafCullingMode::SolidAggressive => "solid_aggressive",
            LeafCullingMode::Hollow => "hollow",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LeafCullingMode::Off => "Off",
            LeafCullingMode::Hidden => "Hidden Faces",
            LeafCullingMode::Solid => "Solid",
            LeafCullingMode::SolidAggressive => "Solid Aggressive",
            LeafCullingMode::Hollow => "Hollow",
        }
    }

    /// Whether this mode draws interior leaf faces in the solid layer.
    pub fn is_interior(self) -> bool {
        matches!(
            self,
            LeafCullingMode::Solid | LeafCullingMode::SolidAggressive
        )
    }

    #[doc(hidden)]
    pub fn parse(s: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|mode| mode.name() == s)
            .unwrap_or_default()
    }
}
