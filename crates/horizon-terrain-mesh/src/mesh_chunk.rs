//! The borrowed, zero-copy chunk view used by the CPU section mesher.

/// Borrowed section storage used by the mesher.
pub trait MeshSectionView {
    /// Whether every block in this section is air.
    fn is_uniform_air(&self) -> bool;

    /// Unpack into the mesher's reusable center-section buffer.
    fn unpack(&self, out: &mut [u32; 4096]);

    /// Copy a contiguous block-state run from section storage.
    fn get_run(&self, at: usize, out: &mut [u32]);

    /// Copy the corresponding light values from section storage.
    fn copy_light_run(&self, at: usize, out: &mut [u8]);
}

/// The client chunk reads the section mesher needs, without taking ownership or copying chunk data.
pub trait MeshChunk {
    /// Chunk coordinates in columns.
    fn x(&self) -> i32;
    fn z(&self) -> i32;

    /// The minimum block Y held by this column.
    fn min_y(&self) -> i32;

    /// The section containing `world_y`, or its outside-column light value.
    fn section_at(&self, world_y: i32) -> Result<&dyn MeshSectionView, u8>;

    /// A block state at chunk-local coordinates.
    fn get(&self, x: i32, y: i32, z: i32) -> u32;

    /// The packed light at chunk-local coordinates.
    fn light(&self, x: i32, y: i32, z: i32) -> u8;

    /// The quart-biome id selected at block coordinates.
    fn biome(&self, x: i32, y: i32, z: i32) -> u16;
}

/// The packed section index of a block at local coordinates.
#[inline]
pub fn block_index(x: i32, y: i32, z: i32) -> usize {
    ((y & 15) << 8 | (z & 15) << 4 | (x & 15)) as usize
}
