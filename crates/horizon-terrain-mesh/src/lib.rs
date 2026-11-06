//! Portable terrain section meshes, side sorting, and compact vertex layouts.
pub mod packed;
mod sides;

pub use horizon_terrain_data::{TILED_UV_STEPS, TerrainVertex, UV_STEPS};
pub use sides::{Part, Sides, group_parts, sort_tinted};

/// Set on a terrain vertex's sprite layer when its texture alpha must be ignored.
pub const OPAQUE_LAYER: u16 = 1 << 15;

/// VisGraph's fully connected visibility value.
pub const ALL_VISIBLE: u64 = (1 << 36) - 1;

/// The mesh data produced by Horizon's section mesher and consumed by render backends.
pub struct SectionMesh {
    pub solid: Vec<TerrainVertex>,
    /// The buckets `solid` is sorted in by the side its quads face: empty
    /// until `sort_solid`, which the meshing workers call; `solid` is in
    /// model order before.
    pub sides: Sides,
    pub translucent: Vec<TerrainVertex>,
    /// `solid` in the compact layout (`packed`), when the renderer reads it and it is
    /// smaller; empty otherwise. Made by `sort_solid`.
    pub packed: Vec<[u32; 6]>,
    /// The tint of each solid quad as the mesher multiplied it (`packed::UNKNOWN_TINT` where
    /// it does not say), for `packed`; dropped once that is made.
    solid_tints: Vec<u32>,
    /// VisGraph: bit `a * 6 + b` is set when faces `a` and `b` (Direction
    /// order) are connected through non-opaque blocks.
    pub visibility: u64,
    /// Blocks with a block-entity renderer (world position, state id), drawn
    /// each frame by `block_entity_render` like vanilla's per-section
    /// `SectionCompiler.Results.blockEntities`.
    pub block_entities: Vec<([i32; 3], u32)>,
    /// Blocks that give off light (world position, state id), for the colour of block light
    /// Horizon's shader packs read (`gfx::block_light`).
    pub lights: Vec<([i32; 3], u32)>,
    /// Sprite layers emitted while building this mesh. Kept distinct before
    /// upload so the renderer does not rescan every terrain vertex.
    pub sprite_layers: Vec<u16>,
    /// Axis-aligned bounding box [min, max] of actual terrain mesh vertices in section-local space [0, 16].
    pub bounds: Option<[glam::Vec3; 2]>,
    /// A far mesh was clipped around finer tiles or resident chunks. Keep
    /// the previous GPU mesh while its replacements fade in.
    pub keep: bool,
}

impl Default for SectionMesh {
    fn default() -> Self {
        Self {
            solid: Vec::new(),
            sides: Sides::default(),
            translucent: Vec::new(),
            packed: Vec::new(),
            solid_tints: Vec::new(),
            visibility: ALL_VISIBLE,
            block_entities: Vec::new(),
            lights: Vec::new(),
            sprite_layers: Vec::new(),
            bounds: None,
            keep: false,
        }
    }
}

impl SectionMesh {
    /// Record a sprite layer used by a quad. Opaque is a vertex-only flag and
    /// is removed here so the renderer can compare the actual atlas layer.
    pub fn note_sprite_layer(&mut self, layer: u16) {
        let layer = layer & !OPAQUE_LAYER;
        if !self.sprite_layers.contains(&layer) {
            self.sprite_layers.push(layer);
        }
    }

    /// Finish the compact layer list after meshing has emitted all faces.
    pub fn finish_sprite_layers(&mut self) {
        self.sprite_layers.sort_unstable();
        self.sprite_layers.dedup();
    }

    /// Padding applied to mesh vertex bounds to prevent clipping at the frustum
    /// boundary when shader packs with vertex displacement (e.g. waving foliage, water) are active.
    pub const BOUNDS_PAD: f32 = 0.5;

    /// Retain geometry capacity after an upload, but none of its old contents.
    pub fn clear(&mut self) {
        self.solid.clear();
        self.translucent.clear();
        self.packed.clear();
        self.solid_tints.clear();
        self.block_entities.clear();
        self.lights.clear();
        self.sprite_layers.clear();
        self.sides = Sides::default();
        self.visibility = ALL_VISIBLE;
        self.bounds = None;
        self.keep = false;
    }

    /// Tint scratch storage used while the mesher emits solid quads.
    pub fn solid_tints_mut(&mut self) -> &mut Vec<u32> {
        &mut self.solid_tints
    }

    /// Approximate capacity retained by this mesh's reusable buffers.
    pub fn capacity_bytes(&self) -> usize {
        self.solid.capacity() * std::mem::size_of::<TerrainVertex>()
            + self.translucent.capacity() * std::mem::size_of::<TerrainVertex>()
            + self.packed.capacity() * std::mem::size_of::<[u32; 6]>()
            + self.solid_tints.capacity() * std::mem::size_of::<u32>()
            + self.block_entities.capacity() * std::mem::size_of::<([i32; 3], u32)>()
            + self.lights.capacity() * std::mem::size_of::<([i32; 3], u32)>()
            + self.sprite_layers.capacity() * std::mem::size_of::<u16>()
    }

    /// Sorts the solid quads by the side they face, for the renderer to
    /// leave out those facing away from the eye (`sides`).
    /// Then lays them out compactly (`packed`) while the renderer reads that.
    pub fn sort_solid(&mut self) {
        let quads = self.solid.len() / 4;
        if !self.solid_tints.is_empty() {
            self.solid_tints.resize(quads, packed::UNKNOWN_TINT);
        }
        self.sides = sides::sort_tinted(&mut self.solid, &mut self.solid_tints);
        self.bounds = self.compute_bounds();
        self.packed.clear();
        if packed::packing() && quads > 0 && self.solid_tints.len() == quads {
            self.packed = packed::layout(&self.solid, &self.solid_tints);
        }
        self.solid_tints.clear();
    }

    /// Computes the tight axis-aligned bounding box [min, max] of actual terrain geometry vertices.
    pub fn compute_bounds(&self) -> Option<[glam::Vec3; 2]> {
        if self.solid.is_empty() && self.translucent.is_empty() {
            return None;
        }
        let mut min = glam::Vec3::splat(f32::INFINITY);
        let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
        for v in &self.solid {
            let p = glam::Vec3::from(v.pos);
            min = min.min(p);
            max = max.max(p);
        }
        for v in &self.translucent {
            let p = glam::Vec3::from(v.pos);
            min = min.min(p);
            max = max.max(p);
        }
        let pad = glam::Vec3::splat(Self::BOUNDS_PAD);
        Some([min - pad, max + pad])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_bounds_empty_solid_and_translucent() {
        let mut mesh = SectionMesh::default();
        assert_eq!(mesh.compute_bounds(), None);

        let v1 = TerrainVertex::new([1.0, 2.0, 3.0], [0.0, 0.0], 0, 0, 0);
        let v2 = TerrainVertex::new([4.0, 5.0, 6.0], [0.0, 0.0], 0, 0, 0);
        mesh.solid.push(v1);
        mesh.solid.push(v2);

        let pad = glam::Vec3::splat(SectionMesh::BOUNDS_PAD);
        assert_eq!(
            mesh.compute_bounds(),
            Some([
                glam::Vec3::new(1.0, 2.0, 3.0) - pad,
                glam::Vec3::new(4.0, 5.0, 6.0) + pad,
            ])
        );

        let v3 = TerrainVertex::new([0.5, 7.0, 8.0], [0.0, 0.0], 0, 0, 0);
        mesh.translucent.push(v3);
        assert_eq!(
            mesh.compute_bounds(),
            Some([
                glam::Vec3::new(0.5, 2.0, 3.0) - pad,
                glam::Vec3::new(4.0, 7.0, 8.0) + pad,
            ])
        );
    }
}
