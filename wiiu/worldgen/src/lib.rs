//! CafeOS bridge from Horizon's real 26.3 proto-chunks to the GX2 renderer.
//!
//! The renderer receives the same shared `TerrainVertex` ABI used by Horizon's
//! desktop client. Block shape selection is intentionally simple in this
//! first bridge; chunk states, registry, seed, height and terrain come from
//! `horizon-world`'s production OverworldGenerator.

use horizon_terrain_data::TerrainVertex;
use horizon_world::{
    level::data::load_registry_with_flags,
    registry::{BlockRegistry, BlockStateId},
    worldgen::generator::{
        FlatSettings, GeneratorData, GeneratorWorker, OverworldGenerator, ProtoChunk, StageTimings,
    },
};
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr},
    path::Path,
    ptr,
    sync::{Mutex, OnceLock},
    thread::{self, JoinHandle},
};

const SECTION_VOLUME: usize = 16 * 16 * 16;
const COLUMN_COUNT: usize = 16 * 16;
const TERRAIN_ATLAS_BYTES: usize = 64 * 32 * 4;
const STATUS_PATH: &str = "/vol/external01/wiiu/apps/HorizonWiiU/worldgen-status.log";

fn write_status(status: &str) {
    let _ = std::fs::write(STATUS_PATH, status.as_bytes());
}

#[derive(Clone, Copy)]
struct Material {
    top: (u8, u8),
    side: (u8, u8),
    bottom: (u8, u8),
}

impl Material {
    fn from_name(name: &str) -> Self {
        if name == "minecraft:grass_block" {
            Self {
                top: (0, 0),
                side: (1, 0),
                bottom: (3, 0),
            }
        } else if name.contains("sand") {
            Self {
                top: (0, 1),
                side: (0, 1),
                bottom: (0, 1),
            }
        } else if name.contains("gravel") {
            Self {
                top: (1, 1),
                side: (1, 1),
                bottom: (1, 1),
            }
        } else if name.contains("log") {
            Self {
                top: (2, 1),
                side: (2, 1),
                bottom: (2, 1),
            }
        } else if name.contains("planks") || name.contains("wood") {
            Self {
                top: (3, 1),
                side: (3, 1),
                bottom: (3, 1),
            }
        } else if name.contains("dirt") || name.contains("terracotta") || name.contains("clay") {
            Self {
                top: (3, 0),
                side: (3, 0),
                bottom: (3, 0),
            }
        } else {
            Self {
                top: (2, 0),
                side: (2, 0),
                bottom: (2, 0),
            }
        }
    }

    fn tile(self, face: Face) -> (u8, u8) {
        match face {
            Face::Up => self.top,
            Face::Down => self.bottom,
            _ => self.side,
        }
    }
}

#[derive(Clone, Copy)]
enum Face {
    Up,
    Down,
    East,
    West,
    South,
    North,
}

impl Face {
    const ALL: [Self; 6] = [
        Self::Up,
        Self::Down,
        Self::East,
        Self::West,
        Self::South,
        Self::North,
    ];

    fn neighbor(self, x: usize, y: usize, z: usize) -> (i32, i32, i32) {
        match self {
            Self::Up => (x as i32, y as i32 + 1, z as i32),
            Self::Down => (x as i32, y as i32 - 1, z as i32),
            Self::East => (x as i32 + 1, y as i32, z as i32),
            Self::West => (x as i32 - 1, y as i32, z as i32),
            Self::South => (x as i32, y as i32, z as i32 + 1),
            Self::North => (x as i32, y as i32, z as i32 - 1),
        }
    }

    fn shade(self) -> u8 {
        match self {
            Self::Up => 255,
            Self::Down => 128,
            Self::East => 220,
            Self::West => 178,
            Self::South => 205,
            Self::North => 190,
        }
    }

    fn corners(self, x: usize, y: i32, z: usize) -> [[f32; 3]; 4] {
        let (x, z) = (x as f32, z as f32);
        let (y0, y1) = (y as f32, y as f32 + 1.0);
        match self {
            Self::Up => [
                [x, y1, z],
                [x + 1.0, y1, z],
                [x + 1.0, y1, z + 1.0],
                [x, y1, z + 1.0],
            ],
            Self::Down => [
                [x, y0, z + 1.0],
                [x + 1.0, y0, z + 1.0],
                [x + 1.0, y0, z],
                [x, y0, z],
            ],
            Self::East => [
                [x + 1.0, y0, z + 1.0],
                [x + 1.0, y1, z + 1.0],
                [x + 1.0, y1, z],
                [x + 1.0, y0, z],
            ],
            Self::West => [[x, y0, z], [x, y1, z], [x, y1, z + 1.0], [x, y0, z + 1.0]],
            Self::South => [
                [x + 1.0, y0, z + 1.0],
                [x + 1.0, y1, z + 1.0],
                [x, y1, z + 1.0],
                [x, y0, z + 1.0],
            ],
            Self::North => [[x, y0, z], [x, y1, z], [x + 1.0, y1, z], [x + 1.0, y0, z]],
        }
    }
}

struct WorldContext {
    worker: GeneratorWorker,
    opaque: Vec<bool>,
    materials: Vec<Material>,
    placement_state: u32,
    edits: HashMap<(i32, i32, i32), u32>,
    spawn: [i32; 2],
    terrain_atlas: Option<Box<[u8]>>,
}

struct WorldChunk {
    vertices: Box<[TerrainVertex]>,
    heights: Box<[i16]>,
    blocks: Box<[u32]>,
    opaque: Box<[bool]>,
    materials: Box<[Material]>,
    min_y: i32,
    height: usize,
    diameter: usize,
    chunk_x: i32,
    chunk_z: i32,
}

static WORLD: OnceLock<Mutex<Option<WorldContext>>> = OnceLock::new();
static PENDING_REGION: OnceLock<Mutex<Option<JoinHandle<Result<usize, String>>>>> = OnceLock::new();

fn world() -> &'static Mutex<Option<WorldContext>> {
    WORLD.get_or_init(|| Mutex::new(None))
}

fn pending_region() -> &'static Mutex<Option<JoinHandle<Result<usize, String>>>> {
    PENDING_REGION.get_or_init(|| Mutex::new(None))
}

const PROFILE_OVERWORLD: u32 = 0;
const PROFILE_SUPERFLAT: u32 = 1;

fn load_world(root: &Path, seed: i64, profile: u32) -> Result<WorldContext, String> {
    let terrain_atlas = match std::fs::read(root.join("wiiu-terrain-atlas.rgba")) {
        Ok(bytes) if bytes.len() == TERRAIN_ATLAS_BYTES => Some(bytes.into_boxed_slice()),
        Ok(bytes) => {
            return Err(format!(
                "prepared Wii U terrain atlas has {} bytes, expected {TERRAIN_ATLAS_BYTES}",
                bytes.len()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("prepared Wii U terrain atlas: {error}")),
    };
    let (registry, flags) = load_registry_with_flags(root)
        .map_err(|error| format!("Horizon block registry: {error}"))?;
    write_status(&format!("registry loaded: {} states\n", registry.len()));
    let registry: &'static BlockRegistry = Box::leak(Box::new(registry));
    let placement_state = (0..registry.len())
        .find(|&index| {
            registry
                .state(BlockStateId(index as u32))
                .is_some_and(|state| state.name == "minecraft:dirt")
        })
        .ok_or_else(|| "Horizon registry has no minecraft:dirt state".to_owned())?
        as u32;
    let materials = (0..registry.len())
        .map(|index| {
            registry
                .state(BlockStateId(index as u32))
                .map(|state| Material::from_name(&state.name))
                .ok_or_else(|| format!("missing Horizon block state {index}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    write_status("registry material map loaded\n");
    let opaque = flags
        .into_iter()
        .map(|(is_air, has_fluid)| !is_air && !has_fluid)
        .collect();
    let data = GeneratorData::load(&root.join("worldgen"), registry)
        .map_err(|error| format!("Horizon worldgen data: {error}"))?;
    write_status("worldgen tables loaded\n");
    let generator = match profile {
        PROFILE_OVERWORLD => OverworldGenerator::without_structures(data, seed),
        PROFILE_SUPERFLAT => OverworldGenerator::flat(
            data,
            seed,
            &FlatSettings {
                layers: vec![
                    ("minecraft:bedrock".to_owned(), 1),
                    ("minecraft:dirt".to_owned(), 2),
                    ("minecraft:grass_block".to_owned(), 1),
                ],
                biome: "minecraft:plains".to_owned(),
                structure_overrides: Some(Vec::new()),
                features: false,
                lakes: false,
            },
            false,
        ),
        _ => return Err(format!("unknown Horizon world profile {profile}")),
    }
    .map_err(|error| format!("Horizon world generator: {error}"))?;
    write_status("spawn search started\n");
    let spawn = generator
        .spawn_position()
        .map_err(|error| format!("Horizon spawn search: {error}"))?;
    write_status(&format!("spawn found: {},{}\n", spawn[0], spawn[1]));
    let worker = generator.worker();

    Ok(WorldContext {
        worker,
        opaque,
        materials,
        placement_state,
        edits: HashMap::new(),
        spawn,
        terrain_atlas,
    })
}

fn is_opaque(id: u32, opaque: &[bool]) -> bool {
    opaque.get(id as usize).copied().unwrap_or(false)
}

fn emit_face(
    vertices: &mut Vec<TerrainVertex>,
    material: Material,
    face: Face,
    x: usize,
    y: i32,
    z: usize,
) {
    let tile = material.tile(face);
    // The Wii U atlas contains four 16x16 tiles per row. Sample texel centers
    // so point filtering never crosses into a neighboring block texture.
    let inset_u = 0.5 / 64.0;
    let inset_v = 0.5 / 32.0;
    let scale_u = 15.0 / 64.0;
    let scale_v = 15.0 / 32.0;
    let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let shade = u32::from(face.shade());
    let color = 0xFF00_0000 | shade * 0x0001_0101;
    for (position, corner_uv) in face.corners(x, y, z).into_iter().zip(uv) {
        let uv = [
            f32::from(tile.0) * 0.25 + inset_u + corner_uv[0] * scale_u,
            f32::from(tile.1) * 0.5 + inset_v + corner_uv[1] * scale_v,
        ];
        vertices.push(TerrainVertex::new(position, uv, 0, color, 0x00F0));
    }
}

struct ChunkBlocks {
    states: Vec<u32>,
    min_y: i32,
    height: usize,
}

fn proto_blocks(proto: &ProtoChunk) -> Result<ChunkBlocks, String> {
    let min_y = proto.terrain.min_y();
    let section_height = proto.terrain.height();
    if min_y % 16 != 0 || section_height <= 0 || section_height % 16 != 0 {
        return Err(format!(
            "unexpected Horizon chunk bounds: {min_y} + {section_height}"
        ));
    }

    let height = section_height as usize;
    let mut states = Vec::with_capacity(height * COLUMN_COUNT);
    let mut section = [0u32; SECTION_VOLUME];
    for section_y in (min_y..min_y + section_height).step_by(16) {
        if !proto.terrain.section_states(section_y, &mut section) {
            return Err(format!("Horizon chunk has no section at y={section_y}"));
        }
        states.extend_from_slice(&section);
    }

    Ok(ChunkBlocks {
        states,
        min_y,
        height,
    })
}

fn block_index(
    diameter: usize,
    min_y: i32,
    height: usize,
    x: i32,
    y: i32,
    z: i32,
) -> Option<usize> {
    if !(0..diameter as i32).contains(&x)
        || !(min_y..min_y + height as i32).contains(&y)
        || !(0..diameter as i32).contains(&z)
    {
        None
    } else {
        let local_y = (y - min_y) as usize;
        Some((local_y * diameter + z as usize) * diameter + x as usize)
    }
}

fn block_state(
    blocks: &[u32],
    diameter: usize,
    min_y: i32,
    height: usize,
    x: i32,
    y: i32,
    z: i32,
) -> u32 {
    block_index(diameter, min_y, height, x, y, z)
        .and_then(|index| blocks.get(index).copied())
        .unwrap_or(0)
}

fn surface_heights(
    blocks: &[u32],
    diameter: usize,
    min_y: i32,
    height: usize,
    opaque: &[bool],
) -> Box<[i16]> {
    let mut heights = vec![min_y as i16; diameter * diameter];
    for z in 0..diameter {
        for x in 0..diameter {
            for y in (min_y..min_y + height as i32).rev() {
                let state = block_state(blocks, diameter, min_y, height, x as i32, y, z as i32);
                if is_opaque(state, opaque) {
                    heights[z * diameter + x] = (y + 1) as i16;
                    break;
                }
            }
        }
    }
    heights.into_boxed_slice()
}

fn mesh_blocks(
    blocks: &[u32],
    diameter: usize,
    min_y: i32,
    height: usize,
    opaque: &[bool],
    materials: &[Material],
) -> Box<[TerrainVertex]> {
    let state_at =
        |x: i32, y: i32, z: i32| -> u32 { block_state(blocks, diameter, min_y, height, x, y, z) };

    let mut vertices = Vec::with_capacity(24 * 1024);
    for y in min_y..min_y + height as i32 {
        for z in 0..diameter {
            for x in 0..diameter {
                let state = state_at(x as i32, y, z as i32);
                if !is_opaque(state, opaque) {
                    continue;
                }
                let Some(material) = materials.get(state as usize).copied() else {
                    continue;
                };
                for face in Face::ALL {
                    let (nx, ny, nz) = face.neighbor(x, (y - min_y) as usize, z);
                    let neighbor = (nx, ny + min_y, nz);
                    if !is_opaque(state_at(neighbor.0, neighbor.1, neighbor.2), opaque) {
                        emit_face(&mut vertices, material, face, x, y, z);
                    }
                }
            }
        }
    }
    vertices.into_boxed_slice()
}

fn apply_chunk_edits(
    blocks: &mut [u32],
    chunk_x: i32,
    chunk_z: i32,
    min_y: i32,
    height: usize,
    edits: &HashMap<(i32, i32, i32), u32>,
) {
    let origin_x = chunk_x.saturating_mul(16);
    let origin_z = chunk_z.saturating_mul(16);
    for (&(x, y, z), &state) in edits {
        let (Some(local_x), Some(local_z)) = (x.checked_sub(origin_x), z.checked_sub(origin_z))
        else {
            continue;
        };
        if let Some(index) = block_index(16, min_y, height, local_x, y, local_z) {
            blocks[index] = state;
        }
    }
}

fn set_block_data(
    blocks: &mut [u32],
    diameter: usize,
    min_y: i32,
    height: usize,
    x: i32,
    y: i32,
    z: i32,
    state: u32,
) -> Option<u32> {
    let index = block_index(diameter, min_y, height, x, y, z)?;
    let old = *blocks.get(index)?;
    blocks[index] = state;
    Some(old)
}

fn floor_block(value: f32) -> Option<i32> {
    if !value.is_finite() || value < i32::MIN as f32 || value >= i32::MAX as f32 {
        return None;
    }
    let truncated = value as i32;
    Some(if value < truncated as f32 {
        truncated - 1
    } else {
        truncated
    })
}

fn player_aabb_clear(
    blocks: &[u32],
    diameter: usize,
    min_y: i32,
    world_origin_x: i32,
    world_origin_z: i32,
    height: usize,
    opaque: &[bool],
    center_x: f32,
    eye_y: f32,
    center_z: f32,
    half_width: f32,
    body_height: f32,
) -> bool {
    if ![center_x, eye_y, center_z, half_width, body_height]
        .iter()
        .all(|value| value.is_finite())
        || half_width <= 0.0
        || body_height <= 0.0
        || body_height <= 1.62
    {
        return false;
    }
    let epsilon = 0.001;
    let min_x = center_x - world_origin_x as f32 - half_width + epsilon;
    let max_x = center_x - world_origin_x as f32 + half_width - epsilon;
    let min_z = center_z - world_origin_z as f32 - half_width + epsilon;
    let max_z = center_z - world_origin_z as f32 + half_width - epsilon;
    let min_body_y = eye_y - 1.62 + epsilon;
    let max_body_y = eye_y + body_height - 1.62 - epsilon;
    let (Some(min_x), Some(max_x), Some(min_z), Some(max_z), Some(min_body_y), Some(max_body_y)) = (
        floor_block(min_x),
        floor_block(max_x),
        floor_block(min_z),
        floor_block(max_z),
        floor_block(min_body_y),
        floor_block(max_body_y),
    ) else {
        return false;
    };
    for y in min_body_y..=max_body_y {
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                if is_opaque(
                    block_state(blocks, diameter, min_y, height, x, y, z),
                    opaque,
                ) {
                    return false;
                }
            }
        }
    }
    true
}

fn generate_chunk_blocks(
    context: &mut WorldContext,
    chunk_x: i32,
    chunk_z: i32,
) -> Result<ChunkBlocks, String> {
    let position = horizon_core::position::ChunkPos::new(chunk_x, chunk_z);
    write_status(&format!("chunk generation started: {chunk_x},{chunk_z}\n"));
    let proto = context
        .worker
        .generate_proto(position)
        .map_err(|error| format!("Horizon chunk generation: {error}"))?;
    write_status("proto-chunk generated\n");
    let mut chunk = proto_blocks(&proto)?;
    apply_chunk_edits(
        &mut chunk.states,
        chunk_x,
        chunk_z,
        chunk.min_y,
        chunk.height,
        &context.edits,
    );
    Ok(chunk)
}

fn world_chunk_from_blocks(
    blocks: Vec<u32>,
    min_y: i32,
    height: usize,
    diameter: usize,
    chunk_x: i32,
    chunk_z: i32,
    context: &WorldContext,
) -> WorldChunk {
    let heights = surface_heights(&blocks, diameter, min_y, height, &context.opaque);
    let vertices = mesh_blocks(
        &blocks,
        diameter,
        min_y,
        height,
        &context.opaque,
        &context.materials,
    );
    WorldChunk {
        vertices,
        heights,
        blocks: blocks.into_boxed_slice(),
        opaque: context.opaque.clone().into_boxed_slice(),
        materials: context.materials.clone().into_boxed_slice(),
        min_y,
        height,
        diameter,
        chunk_x,
        chunk_z,
    }
}

fn raycast_blocks(
    blocks: &[u32],
    diameter: usize,
    min_y: i32,
    height: usize,
    opaque: &[bool],
    origin: [f32; 3],
    direction: [f32; 3],
    max_distance: f32,
) -> Option<[i32; 6]> {
    if !origin
        .iter()
        .chain(direction.iter())
        .all(|value| value.is_finite())
        || !max_distance.is_finite()
        || max_distance <= 0.0
    {
        return None;
    }
    let length =
        (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
            .sqrt();
    if length <= f32::EPSILON {
        return None;
    }
    let direction = [
        direction[0] / length,
        direction[1] / length,
        direction[2] / length,
    ];
    let mut cell = [0i32; 3];
    for axis in 0..3 {
        cell[axis] = floor_block(origin[axis])?;
    }

    let step = direction.map(|component| {
        if component > 0.0 {
            1
        } else if component < 0.0 {
            -1
        } else {
            0
        }
    });
    let mut next = [f32::INFINITY; 3];
    let mut delta = [f32::INFINITY; 3];
    for axis in 0..3 {
        if step[axis] != 0 {
            let boundary = if step[axis] > 0 {
                cell[axis] as f32 + 1.0
            } else {
                cell[axis] as f32
            };
            next[axis] = (boundary - origin[axis]) / direction[axis];
            delta[axis] = 1.0 / direction[axis].abs();
        }
    }

    if is_opaque(
        block_state(blocks, diameter, min_y, height, cell[0], cell[1], cell[2]),
        opaque,
    ) {
        return Some([cell[0], cell[1], cell[2], cell[0], cell[1], cell[2]]);
    }
    loop {
        let axis = if next[0] <= next[1] && next[0] <= next[2] {
            0
        } else if next[1] <= next[2] {
            1
        } else {
            2
        };
        let distance = next[axis];
        if !distance.is_finite() || distance > max_distance {
            return None;
        }
        let previous = cell;
        cell[axis] += step[axis];
        next[axis] += delta[axis];
        let state = block_state(blocks, diameter, min_y, height, cell[0], cell[1], cell[2]);
        if is_opaque(state, opaque) {
            return Some([
                cell[0],
                cell[1],
                cell[2],
                previous[0],
                previous[1],
                previous[2],
            ]);
        }
    }
}

fn copy_error(error: &str, output: *mut c_char, capacity: u32) {
    if output.is_null() || capacity == 0 {
        return;
    }
    let capacity = capacity as usize;
    let bytes = error.as_bytes();
    let count = bytes.len().min(capacity - 1);
    // SAFETY: the C caller provides an output buffer with `capacity` writable bytes.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), output.cast::<u8>(), count);
        output.add(count).write(0);
    }
}

/// Load Horizon's prepared 26.3 block registry and terrain generator.
///
/// `resources` points at a prepared client resource directory containing
/// `facts.json`, `worldgen/`, and the generator's biome parameter tables.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_init(
    resources: *const c_char,
    seed: i64,
    profile: u32,
    spawn_out: *mut i32,
    error_out: *mut c_char,
    error_capacity: u32,
) -> i32 {
    write_status("world init entered\n");
    if resources.is_null() || spawn_out.is_null() {
        copy_error("null world-init argument", error_out, error_capacity);
        return 0;
    }
    // SAFETY: C passes a nul-terminated path for the duration of this call.
    let resources = unsafe { CStr::from_ptr(resources) };
    let Ok(resources) = resources.to_str() else {
        copy_error("resource path is not UTF-8", error_out, error_capacity);
        return 0;
    };

    let mut loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    write_status("world lock acquired\n");
    if loaded.is_none() {
        match load_world(Path::new(resources), seed, profile) {
            Ok(context) => *loaded = Some(context),
            Err(error) => {
                copy_error(&error, error_out, error_capacity);
                write_status(&format!("world init failed: {error}\n"));
                return 0;
            }
        }
    }
    let Some(context) = loaded.as_ref() else {
        copy_error(
            "Horizon world initialization did not complete",
            error_out,
            error_capacity,
        );
        return 0;
    };
    // SAFETY: C provides space for the two block coordinates.
    unsafe {
        spawn_out.write(context.spawn[0]);
        spawn_out.add(1).write(context.spawn[1]);
    }
    1
}

/// Generate one actual Horizon overworld proto-chunk and convert its runtime
/// state IDs to visible opaque block faces for GX2. Fluids are omitted for now.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_generate_chunk(
    chunk_x: i32,
    chunk_z: i32,
    error_out: *mut c_char,
    error_capacity: u32,
) -> *mut c_void {
    let mut loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(context) = loaded.as_mut() else {
        copy_error(
            "Horizon world is not initialized",
            error_out,
            error_capacity,
        );
        return ptr::null_mut();
    };
    let chunk = match generate_chunk_blocks(context, chunk_x, chunk_z) {
        Ok(chunk) => chunk,
        Err(error) => {
            copy_error(&error, error_out, error_capacity);
            return ptr::null_mut();
        }
    };
    let world_chunk = world_chunk_from_blocks(
        chunk.states,
        chunk.min_y,
        chunk.height,
        16,
        chunk_x,
        chunk_z,
        context,
    );
    let min_height = world_chunk
        .heights
        .iter()
        .min()
        .copied()
        .unwrap_or_default();
    let max_height = world_chunk
        .heights
        .iter()
        .max()
        .copied()
        .unwrap_or_default();
    write_status(&format!(
        "chunk meshed: {chunk_x},{chunk_z} vertices={} surface_y={min_height}..{max_height}\n",
        world_chunk.vertices.len()
    ));
    Box::into_raw(Box::new(world_chunk)).cast::<c_void>()
}

/// Generate a bounded square of Horizon proto-chunks and combine their meshes.
/// Vertices are region-relative; the returned chunk position is the region origin.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_generate_region(
    center_x: i32,
    center_z: i32,
    radius: u32,
    error_out: *mut c_char,
    error_capacity: u32,
) -> *mut c_void {
    if radius > 1 {
        copy_error("Horizon region radius exceeds 1", error_out, error_capacity);
        return ptr::null_mut();
    }
    let radius = radius as i32;
    let Some(origin_x) = center_x.checked_sub(radius) else {
        copy_error("Horizon region x range overflow", error_out, error_capacity);
        return ptr::null_mut();
    };
    let Some(origin_z) = center_z.checked_sub(radius) else {
        copy_error("Horizon region z range overflow", error_out, error_capacity);
        return ptr::null_mut();
    };
    let Some(end_x) = center_x.checked_add(radius) else {
        copy_error("Horizon region x range overflow", error_out, error_capacity);
        return ptr::null_mut();
    };
    let Some(end_z) = center_z.checked_add(radius) else {
        copy_error("Horizon region z range overflow", error_out, error_capacity);
        return ptr::null_mut();
    };

    let mut loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(context) = loaded.as_mut() else {
        copy_error(
            "Horizon world is not initialized",
            error_out,
            error_capacity,
        );
        return ptr::null_mut();
    };

    let side = (radius * 2 + 1) as usize;
    let diameter = side * 16;
    let mut min_y = None;
    let mut height = None;
    let mut blocks = Vec::new();
    for chunk_z in origin_z..=end_z {
        for chunk_x in origin_x..=end_x {
            let chunk = match generate_chunk_blocks(context, chunk_x, chunk_z) {
                Ok(chunk) => chunk,
                Err(error) => {
                    copy_error(&error, error_out, error_capacity);
                    return ptr::null_mut();
                }
            };
            if min_y.is_some_and(|expected| expected != chunk.min_y)
                || height.is_some_and(|expected| expected != chunk.height)
            {
                copy_error(
                    "Horizon chunks have inconsistent vertical bounds",
                    error_out,
                    error_capacity,
                );
                return ptr::null_mut();
            }
            min_y = Some(chunk.min_y);
            height = Some(chunk.height);
            if blocks.is_empty() {
                blocks.resize(chunk.height * diameter * diameter, 0);
            }
            let local_chunk_x = ((chunk_x - origin_x) * 16) as usize;
            let local_chunk_z = ((chunk_z - origin_z) * 16) as usize;
            for local_y in 0..chunk.height {
                for z in 0..16 {
                    let source_row = local_y * 256 + z * 16;
                    let target_row =
                        (local_y * diameter + local_chunk_z + z) * diameter + local_chunk_x;
                    blocks[target_row..target_row + 16]
                        .copy_from_slice(&chunk.states[source_row..source_row + 16]);
                }
            }
        }
    }
    let min_y = min_y.unwrap_or(-64);
    let height = height.unwrap_or_default();
    let world_chunk =
        world_chunk_from_blocks(blocks, min_y, height, diameter, origin_x, origin_z, context);
    let min_height = world_chunk
        .heights
        .iter()
        .min()
        .copied()
        .unwrap_or_default();
    let max_height = world_chunk
        .heights
        .iter()
        .max()
        .copied()
        .unwrap_or_default();
    let timings = context.worker.stage_timings();
    write_status(&format!(
        "region meshed: center={center_x},{center_z} chunks={} vertices={} surface_y={min_height}..{max_height} atlas={} gen_ms=biomes:{},noise:{},surface:{},structures:{},carvers:{}\n",
        side * side,
        world_chunk.vertices.len(),
        if context.terrain_atlas.is_some() { "local" } else { "placeholder" },
        timings.biomes.as_millis(),
        timings.noise.as_millis(),
        timings.surface.as_millis(),
        timings.structures.as_millis(),
        timings.carvers.as_millis()
    ));
    Box::into_raw(Box::new(world_chunk)).cast::<c_void>()
}

/// Start generating a region on Cafe std's native worker thread.
/// Returns 1 when started, 0 when another region is pending, and -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_request_region(
    center_x: i32,
    center_z: i32,
    radius: u32,
    error_out: *mut c_char,
    error_capacity: u32,
) -> i32 {
    let mut pending = pending_region()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if pending.is_some() {
        return 0;
    }

    let worker = thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(move || {
            let mut error = [0 as c_char; 256];
            let chunk = horizon_wiiu_world_generate_region(
                center_x,
                center_z,
                radius,
                error.as_mut_ptr(),
                error.len() as u32,
            );
            if chunk.is_null() {
                // SAFETY: the synchronous generator writes a NUL-terminated error
                // into this live, zero-initialized buffer when it fails.
                let message = unsafe { CStr::from_ptr(error.as_ptr()) }
                    .to_string_lossy()
                    .into_owned();
                Err(message)
            } else {
                Ok(chunk as usize)
            }
        });

    match worker {
        Ok(worker) => {
            *pending = Some(worker);
            1
        }
        Err(error) => {
            copy_error(
                &format!("could not start Horizon region worker: {error}"),
                error_out,
                error_capacity,
            );
            -1
        }
    }
}

/// Poll a background region request: 0 while running, 1 when complete, -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_take_region_result(
    chunk_out: *mut *mut c_void,
    error_out: *mut c_char,
    error_capacity: u32,
) -> i32 {
    if chunk_out.is_null() {
        copy_error("missing region result pointer", error_out, error_capacity);
        return -1;
    }
    // SAFETY: the C caller provides one writable chunk pointer.
    unsafe { chunk_out.write(ptr::null_mut()) };

    let worker = {
        let mut pending = pending_region()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !pending.as_ref().is_some_and(JoinHandle::is_finished) {
            return 0;
        }
        pending.take()
    };
    let Some(worker) = worker else {
        return 0;
    };

    match worker.join() {
        Ok(Ok(chunk)) => {
            // SAFETY: the integer is the pointer returned by the synchronous
            // region generator, preserved only to cross Cafe's thread boundary.
            unsafe { chunk_out.write(chunk as *mut c_void) };
            1
        }
        Ok(Err(error)) => {
            copy_error(&error, error_out, error_capacity);
            -1
        }
        Err(_) => {
            copy_error("Horizon region worker panicked", error_out, error_capacity);
            -1
        }
    }
}

/// Borrow the generated chunk's Horizon vertex array until `free_chunk`.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_chunk_vertices(
    chunk: *const c_void,
    vertex_count_out: *mut u32,
) -> *const TerrainVertex {
    if chunk.is_null() || vertex_count_out.is_null() {
        return ptr::null();
    }
    // SAFETY: `chunk` came from a world chunk/region function and remains live.
    let chunk = unsafe { &*chunk.cast::<WorldChunk>() };
    let Ok(vertex_count) = u32::try_from(chunk.vertices.len()) else {
        return ptr::null();
    };
    // SAFETY: the C caller provides a writable vertex-count pointer.
    unsafe { vertex_count_out.write(vertex_count) };
    chunk.vertices.as_ptr()
}

/// Borrow the prepared 64x32 RGBA terrain atlas until the world context closes.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_terrain_atlas() -> *const u8 {
    let loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loaded
        .as_ref()
        .and_then(|context| context.terrain_atlas.as_ref())
        .map_or(ptr::null(), |atlas| atlas.as_ptr())
}

/// Copy worker timing totals as chunks, biomes, noise, surface, structures, carvers (microseconds).
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_stage_timings(output: *mut u64) -> i32 {
    if output.is_null() {
        return 0;
    }
    let loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(context) = loaded.as_ref() else {
        return 0;
    };
    let StageTimings {
        chunks,
        biomes,
        noise,
        surface,
        structures,
        carvers,
    } = context.worker.stage_timings();
    let micros =
        |duration: std::time::Duration| u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
    let values = [
        chunks,
        micros(biomes),
        micros(noise),
        micros(surface),
        micros(structures),
        micros(carvers),
    ];
    // SAFETY: the C caller provides six writable u64 entries.
    unsafe { ptr::copy_nonoverlapping(values.as_ptr(), output, values.len()) };
    1
}

/// Copy the generated Horizon world-surface heights for camera grounding.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_chunk_heights(
    chunk: *const c_void,
    heights_out: *mut i16,
    capacity: u32,
) -> i32 {
    if chunk.is_null() || heights_out.is_null() {
        return 0;
    }
    // SAFETY: `chunk` came from a world chunk/region function and remains live.
    let chunk = unsafe { &*chunk.cast::<WorldChunk>() };
    let Ok(height_count) = u32::try_from(chunk.heights.len()) else {
        return 0;
    };
    if capacity < height_count {
        return 0;
    }
    // SAFETY: C supplies `capacity` writable height entries after the check.
    unsafe { ptr::copy_nonoverlapping(chunk.heights.as_ptr(), heights_out, height_count as usize) };
    1
}

/// Return the first opaque Horizon block intersected by a normalized ray.
/// Output is `[hit_x, hit_y, hit_z, adjacent_x, adjacent_y, adjacent_z]`.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_raycast(
    chunk: *const c_void,
    origin: *const f32,
    direction: *const f32,
    max_distance: f32,
    hit_out: *mut i32,
) -> i32 {
    if chunk.is_null() || origin.is_null() || direction.is_null() || hit_out.is_null() {
        return 0;
    }
    // SAFETY: C keeps the generated region alive and provides three floats for
    // both vectors plus six writable hit coordinates.
    let chunk = unsafe { &*chunk.cast::<WorldChunk>() };
    let origin = unsafe { core::slice::from_raw_parts(origin, 3) };
    let direction = unsafe { core::slice::from_raw_parts(direction, 3) };
    let (Some(world_origin_x), Some(world_origin_z)) =
        (chunk.chunk_x.checked_mul(16), chunk.chunk_z.checked_mul(16))
    else {
        return 0;
    };
    let Some(mut hit) = raycast_blocks(
        &chunk.blocks,
        chunk.diameter,
        chunk.min_y,
        chunk.height,
        &chunk.opaque,
        [
            origin[0] - world_origin_x as f32,
            origin[1],
            origin[2] - world_origin_z as f32,
        ],
        [direction[0], direction[1], direction[2]],
        max_distance,
    ) else {
        return 0;
    };
    hit[0] += world_origin_x;
    hit[2] += world_origin_z;
    hit[3] += world_origin_x;
    hit[5] += world_origin_z;
    // SAFETY: the C caller provides six writable block coordinates.
    unsafe { ptr::copy_nonoverlapping(hit.as_ptr(), hit_out, hit.len()) };
    1
}

/// Test the player's rectangular body against the loaded Horizon block states.
/// `eye_y` is the camera eye; body height is measured upward from player feet.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_player_clear(
    chunk: *const c_void,
    center_x: f32,
    eye_y: f32,
    center_z: f32,
    half_width: f32,
    body_height: f32,
) -> i32 {
    if chunk.is_null() {
        return 0;
    }
    // SAFETY: C retains this generated region for the duration of the query.
    let chunk = unsafe { &*chunk.cast::<WorldChunk>() };
    let (Some(world_origin_x), Some(world_origin_z)) =
        (chunk.chunk_x.checked_mul(16), chunk.chunk_z.checked_mul(16))
    else {
        return 0;
    };
    player_aabb_clear(
        &chunk.blocks,
        chunk.diameter,
        chunk.min_y,
        world_origin_x,
        world_origin_z,
        chunk.height,
        &chunk.opaque,
        center_x,
        eye_y,
        center_z,
        half_width,
        body_height,
    ) as i32
}

/// Replace one block in a loaded region and rebuild its terrain mesh.
/// Returns 1 when changed, 0 when unchanged/outside the loaded region, or -1
/// when the world is unavailable or the requested state is not opaque.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_set_block(
    chunk: *mut c_void,
    world_x: i32,
    y: i32,
    world_z: i32,
    state: u32,
) -> i32 {
    if chunk.is_null() {
        return -1;
    }
    let mut loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(context) = loaded.as_mut() else {
        return -1;
    };
    // SAFETY: C passes a live region produced by this module and serializes
    // region replacement and edits on the renderer thread.
    let chunk = unsafe { &mut *chunk.cast::<WorldChunk>() };
    if state != 0 && !is_opaque(state, &chunk.opaque) {
        return -1;
    }
    let Some(origin_x) = chunk.chunk_x.checked_mul(16) else {
        return 0;
    };
    let Some(origin_z) = chunk.chunk_z.checked_mul(16) else {
        return 0;
    };
    let (Some(local_x), Some(local_z)) =
        (world_x.checked_sub(origin_x), world_z.checked_sub(origin_z))
    else {
        return 0;
    };
    let Some(old_state) = set_block_data(
        &mut chunk.blocks,
        chunk.diameter,
        chunk.min_y,
        chunk.height,
        local_x,
        y,
        local_z,
        state,
    ) else {
        return 0;
    };
    if old_state == state {
        return 0;
    }
    context.edits.insert((world_x, y, world_z), state);
    chunk.heights = surface_heights(
        &chunk.blocks,
        chunk.diameter,
        chunk.min_y,
        chunk.height,
        &chunk.opaque,
    );
    chunk.vertices = mesh_blocks(
        &chunk.blocks,
        chunk.diameter,
        chunk.min_y,
        chunk.height,
        &chunk.opaque,
        &chunk.materials,
    );
    write_status(&format!(
        "block edited: {world_x},{y},{world_z} {old_state}->{state} vertices={}\n",
        chunk.vertices.len()
    ));
    1
}

/// The first opaque minecraft:dirt state is used for the prototype placement slot.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_placement_state() -> u32 {
    let loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loaded.as_ref().map_or(0, |context| context.placement_state)
}

/// Get the generated chunk coordinates for renderer-relative positioning.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_chunk_position(chunk: *const c_void, output: *mut i32) -> i32 {
    if chunk.is_null() || output.is_null() {
        return 0;
    }
    // SAFETY: `chunk` came from a world chunk/region function and remains live.
    let chunk = unsafe { &*chunk.cast::<WorldChunk>() };
    // SAFETY: C provides two writable i32 values.
    unsafe {
        output.write(chunk.chunk_x);
        output.add(1).write(chunk.chunk_z);
    }
    1
}

/// Release a chunk and its renderer vertex array.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_free_chunk(chunk: *mut c_void) {
    if !chunk.is_null() {
        // SAFETY: `chunk` was returned by a world chunk/region function once.
        unsafe { drop(Box::from_raw(chunk.cast::<WorldChunk>())) };
    }
}

/// Return the spawn coordinate chosen by the production Horizon generator.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_spawn(output: *mut i32) -> i32 {
    if output.is_null() {
        return 0;
    }
    let loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(context) = loaded.as_ref() else {
        return 0;
    };
    // SAFETY: C provides two writable i32 values.
    unsafe {
        output.write(context.spawn[0]);
        output.add(1).write(context.spawn[1]);
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn materials() -> Vec<Material> {
        vec![Material::from_name("minecraft:stone"); 2]
    }

    #[test]
    fn voxel_ray_hits_nearest_block_and_reports_previous_air_cell() {
        let diameter = 16;
        let min_y = 0;
        let height = 4;
        let mut blocks = vec![0; diameter * diameter * height];
        set_block_data(&mut blocks, diameter, min_y, height, 8, 1, 6, 1).unwrap();
        set_block_data(&mut blocks, diameter, min_y, height, 8, 1, 9, 1).unwrap();

        assert_eq!(
            raycast_blocks(
                &blocks,
                diameter,
                min_y,
                height,
                &[false, true],
                [8.5, 1.5, 4.5],
                [0.0, 0.0, 1.0],
                2.0,
            ),
            Some([8, 1, 6, 8, 1, 5])
        );
        assert_eq!(
            raycast_blocks(
                &blocks,
                diameter,
                min_y,
                height,
                &[false, true],
                [8.5, 1.5, 4.5],
                [0.0, 0.0, 1.0],
                1.0,
            ),
            None
        );
    }

    #[test]
    fn edited_horizon_states_change_surface_and_mesh_and_survive_region_reload() {
        let diameter = 16;
        let min_y = 0;
        let height = 4;
        let opaque = [false, true];
        let mut blocks = vec![0; diameter * diameter * height];
        set_block_data(&mut blocks, diameter, min_y, height, 8, 0, 8, 1).unwrap();
        let original_mesh = mesh_blocks(&blocks, diameter, min_y, height, &opaque, &materials());
        assert_eq!(original_mesh.len(), 24);
        assert_eq!(
            surface_heights(&blocks, diameter, min_y, height, &opaque)[8 * diameter + 8],
            1
        );

        assert_eq!(
            set_block_data(&mut blocks, diameter, min_y, height, 8, 1, 8, 1),
            Some(0)
        );
        let placed_mesh = mesh_blocks(&blocks, diameter, min_y, height, &opaque, &materials());
        assert_eq!(placed_mesh.len(), 40);
        assert_eq!(
            surface_heights(&blocks, diameter, min_y, height, &opaque)[8 * diameter + 8],
            2
        );

        let mut edits = HashMap::new();
        edits.insert((8, 1, 8), 1);
        let mut regenerated_chunk = vec![0; 16 * 16 * height];
        apply_chunk_edits(&mut regenerated_chunk, 0, 0, min_y, height, &edits);
        assert_eq!(
            block_state(&regenerated_chunk, 16, min_y, height, 8, 1, 8),
            1
        );

        assert_eq!(
            set_block_data(&mut blocks, diameter, min_y, height, 8, 1, 8, 0),
            Some(1)
        );
        let broken_mesh = mesh_blocks(&blocks, diameter, min_y, height, &opaque, &materials());
        assert_eq!(broken_mesh.len(), 24);
        assert_eq!(
            surface_heights(&blocks, diameter, min_y, height, &opaque)[8 * diameter + 8],
            1
        );
    }

    #[test]
    fn player_body_collision_ignores_floor_contact_but_detects_walls_and_ceilings() {
        let diameter = 16;
        let min_y = 0;
        let height = 5;
        let opaque = [false, true];
        let mut blocks = vec![0; diameter * diameter * height];
        for z in 0..diameter as i32 {
            for x in 0..diameter as i32 {
                set_block_data(&mut blocks, diameter, min_y, height, x, 0, z, 1).unwrap();
            }
        }
        set_block_data(&mut blocks, diameter, min_y, height, 9, 1, 8, 1).unwrap();
        set_block_data(&mut blocks, diameter, min_y, height, 8, 3, 8, 1).unwrap();

        let clear = |x, eye_y, z| {
            player_aabb_clear(
                &blocks, diameter, min_y, 0, 0, height, &opaque, x, eye_y, z, 0.30, 1.80,
            )
        };
        assert!(clear(8.5, 2.62, 8.5));
        assert!(!clear(8.75, 2.62, 8.5));
        assert!(!clear(8.5, 3.62, 8.5));
    }
}
