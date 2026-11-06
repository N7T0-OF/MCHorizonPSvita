//! Small, test-only chunk storage for exercising the portable mesher without
//! introducing a second production chunk representation.
use crate::{MeshChunk, MeshSectionView, block_index};
use std::sync::Arc;

pub const MIN_Y: i32 = -64;

#[derive(Clone, Copy, Debug)]
pub struct ColumnLayout {
    min_y: i32,
    sections: usize,
}
impl ColumnLayout {
    pub fn new(min_y: i32, height: i32) -> Result<Self, String> {
        if min_y % 16 != 0 || height < 16 || height % 16 != 0 {
            return Err("invalid test column layout".into());
        }
        Ok(Self {
            min_y,
            sections: (height / 16) as usize,
        })
    }
    const VANILLA: Self = Self {
        min_y: MIN_Y,
        sections: 24,
    };
}

#[derive(Clone)]
pub enum Light {
    Uniform(u8),
    Full(Box<[u8; 4096]>),
}
impl Light {
    fn get(&self, i: usize) -> u8 {
        match self {
            Self::Uniform(v) => *v,
            Self::Full(values) => values[i],
        }
    }
    fn copy_run(&self, at: usize, out: &mut [u8]) {
        match self {
            Self::Uniform(v) => out.fill(*v),
            Self::Full(values) => out.copy_from_slice(&values[at..at + out.len()]),
        }
    }
}

#[derive(Clone)]
pub struct Section {
    blocks: Box<[u32; 4096]>,
    pub light: Light,
    biomes: [u16; 64],
}
impl Default for Section {
    fn default() -> Self {
        Self {
            blocks: Box::new([0; 4096]),
            light: Light::Uniform(0xF0),
            biomes: [0; 64],
        }
    }
}
impl MeshSectionView for Section {
    fn is_uniform_air(&self) -> bool {
        self.blocks.iter().all(|&state| state == 0)
    }
    fn unpack(&self, out: &mut [u32; 4096]) {
        out.copy_from_slice(&self.blocks[..]);
    }
    fn get_run(&self, at: usize, out: &mut [u32]) {
        out.copy_from_slice(&self.blocks[at..at + out.len()]);
    }
    fn copy_light_run(&self, at: usize, out: &mut [u8]) {
        self.light.copy_run(at, out);
    }
}

pub struct Chunk {
    pub x: i32,
    pub z: i32,
    min_y: i32,
    pub sections: Vec<Arc<Section>>,
}
impl Chunk {
    pub fn empty(x: i32, z: i32) -> Self {
        Self::with_layout(x, z, ColumnLayout::VANILLA)
    }
    pub fn with_layout(x: i32, z: i32, layout: ColumnLayout) -> Self {
        Self {
            x,
            z,
            min_y: layout.min_y,
            sections: (0..layout.sections)
                .map(|_| Arc::new(Section::default()))
                .collect(),
        }
    }
    fn section_index(&self, y: i32) -> Option<usize> {
        usize::try_from((y - self.min_y) >> 4)
            .ok()
            .filter(|&i| i < self.sections.len())
    }
    pub fn set(&mut self, x: i32, y: i32, z: i32, value: u32) -> u32 {
        let Some(section) = self.section_index(y) else {
            return value;
        };
        let section = Arc::make_mut(&mut self.sections[section]);
        std::mem::replace(&mut section.blocks[block_index(x, y, z)], value)
    }
    pub fn set_biome_cell(&mut self, qx: i32, qy: i32, qz: i32, biome: u16) {
        let section_index = (qy >> 2) as usize;
        let local_y = qy & 3;
        let index = (local_y << 4 | (qz & 3) << 2 | (qx & 3)) as usize;
        Arc::make_mut(&mut self.sections[section_index]).biomes[index] = biome;
    }
}
impl MeshChunk for Chunk {
    fn x(&self) -> i32 {
        self.x
    }
    fn z(&self) -> i32 {
        self.z
    }
    fn min_y(&self) -> i32 {
        self.min_y
    }
    fn section_at(&self, y: i32) -> Result<&dyn MeshSectionView, u8> {
        match self.section_index(y) {
            Some(i) => Ok(self.sections[i].as_ref()),
            None if y < self.min_y => Err(0),
            None => Err(0xF0),
        }
    }
    fn get(&self, x: i32, y: i32, z: i32) -> u32 {
        self.section_index(y)
            .map(|i| self.sections[i].blocks[block_index(x, y, z)])
            .unwrap_or(0)
    }
    fn light(&self, x: i32, y: i32, z: i32) -> u8 {
        self.section_index(y)
            .map(|i| self.sections[i].light.get(block_index(x, y, z)))
            .unwrap_or(if y < self.min_y { 0 } else { 0xF0 })
    }
    fn biome(&self, x: i32, y: i32, z: i32) -> u16 {
        let qy = ((y - self.min_y) >> 2).clamp(0, self.sections.len() as i32 * 4 - 1);
        self.sections[(qy >> 2) as usize].biomes
            [((qy & 3) << 4 | ((z & 15) >> 2) << 2 | ((x & 15) >> 2)) as usize]
    }
}
