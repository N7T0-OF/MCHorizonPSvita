//! Horizon's initial simulation primitives, targeting the Minecraft Java Edition version in [`version`].
//!
//! This is not a playable game. Differential checks cover only the operations
//! listed in docs/rust-parity.md; the rest of vanilla remains unimplemented.
#![forbid(unsafe_code)]
#![cfg_attr(feature = "wiiu-core", no_std)]

extern crate alloc;
#[cfg(test)]
extern crate std;

#[cfg(not(feature = "wiiu-core"))]
pub mod biome;
#[cfg(not(feature = "wiiu-core"))]
pub mod carving;
#[cfg(not(feature = "wiiu-core"))]
pub mod density;
#[cfg(not(feature = "wiiu-core"))]
pub mod fast_hash;
#[cfg(not(feature = "wiiu-core"))]
pub mod geometry;
#[cfg(not(feature = "wiiu-core"))]
pub mod identity;
#[cfg(not(feature = "wiiu-core"))]
pub mod jmath;
#[cfg(not(feature = "wiiu-core"))]
pub mod no_ai;
pub mod noise;
#[cfg(not(feature = "wiiu-core"))]
pub mod paletted;
pub mod position;
#[cfg(not(feature = "wiiu-core"))]
pub mod prepared_files;
pub mod random;
#[cfg(not(feature = "wiiu-core"))]
pub mod storage;
#[cfg(not(feature = "wiiu-core"))]
pub mod text;
#[cfg(not(feature = "wiiu-core"))]
pub mod ticks;

#[cfg(not(feature = "wiiu-core"))]
pub mod release;
pub mod version;

pub mod math;

pub use version::VANILLA_VERSION;
