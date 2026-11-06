//! Compatibility exports for the portable section mesher.
pub use horizon_terrain_mesh::mesher::*;
#[cfg(test)]
pub(crate) use horizon_terrain_mesh::sort_tinted;

use crate::chunk::Chunk;
use std::sync::Arc;

/// Snapshot of the 3x3 chunk neighbourhood used by a worker job.
pub type Region = [[Option<Arc<Chunk>>; 3]; 3];

pub fn borrow(region: &Region) -> [[Option<&Chunk>; 3]; 3] {
    std::array::from_fn(|z| std::array::from_fn(|x| region[z][x].as_deref()))
}
