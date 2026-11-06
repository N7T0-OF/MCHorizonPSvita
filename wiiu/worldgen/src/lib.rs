//! CafeOS bridge from Horizon's real 26.3 proto-chunks to the GX2 renderer.
//!
//! The renderer receives the same shared `TerrainVertex` ABI used by Horizon's
//! desktop client. Block shapes and atlas selection are intentionally simple
//! in this first bridge; the chunk states, registry, seed, height and terrain
//! all come from `horizon-world`'s production OverworldGenerator.

use horizon_terrain_data::TerrainVertex;
use horizon_world::{
    level::data::load_registry_with_flags,
    registry::{BlockRegistry, BlockStateId},
    worldgen::generator::{FlatSettings, GeneratorData, OverworldGenerator, ProtoChunk},
};
use std::{
    ffi::{c_char, c_void, CStr},
    path::Path,
    ptr,
    sync::{Mutex, OnceLock},
};

const SECTION_VOLUME: usize = 16 * 16 * 16;
const COLUMN_COUNT: usize = 16 * 16;
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
                bottom: (1, 1),
            }
        } else if name.contains("dirt")
            || name.contains("sand")
            || name.contains("gravel")
            || name.contains("terracotta")
            || name.contains("clay")
        {
            Self {
                top: (1, 1),
                side: (1, 1),
                bottom: (1, 1),
            }
        } else if name.contains("log") || name.contains("wood") || name.contains("planks") {
            Self {
                top: (1, 1),
                side: (1, 1),
                bottom: (1, 1),
            }
        } else {
            Self {
                top: (0, 1),
                side: (0, 1),
                bottom: (0, 1),
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
    generator: OverworldGenerator,
    opaque: Vec<bool>,
    materials: Vec<Material>,
    spawn: [i32; 2],
}

struct WorldChunk {
    vertices: Box<[TerrainVertex]>,
    heights: Box<[i16]>,
    chunk_x: i32,
    chunk_z: i32,
}

static WORLD: OnceLock<Mutex<Option<WorldContext>>> = OnceLock::new();

fn world() -> &'static Mutex<Option<WorldContext>> {
    WORLD.get_or_init(|| Mutex::new(None))
}

const PROFILE_OVERWORLD: u32 = 0;
const PROFILE_SUPERFLAT: u32 = 1;

fn load_world(root: &Path, seed: i64, profile: u32) -> Result<WorldContext, String> {
    let (registry, flags) = load_registry_with_flags(root)
        .map_err(|error| format!("Horizon block registry: {error}"))?;
    write_status(&format!("registry loaded: {} states\n", registry.len()));
    let registry: &'static BlockRegistry = Box::leak(Box::new(registry));
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

    Ok(WorldContext {
        generator,
        opaque,
        materials,
        spawn,
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
    // The bootstrap atlas is a generated 2x2 placeholder atlas. Inset each
    // quad from the tile border to avoid sampling an adjacent placeholder.
    let inset = 1.0 / 16.0;
    let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let shade = u32::from(face.shade());
    let color = 0xFF00_0000 | shade * 0x0001_0101;
    for (position, corner_uv) in face.corners(x, y, z).into_iter().zip(uv) {
        let uv = [
            f32::from(tile.0) * 0.5 + inset + corner_uv[0] * 3.0 / 8.0,
            f32::from(tile.1) * 0.5 + inset + corner_uv[1] * 3.0 / 8.0,
        ];
        vertices.push(TerrainVertex::new(position, uv, 0, color, 0x00F0));
    }
}

fn mesh_chunk(
    proto: &ProtoChunk,
    opaque: &[bool],
    materials: &[Material],
) -> Result<(Box<[TerrainVertex]>, [i16; COLUMN_COUNT]), String> {
    let min_y = proto.terrain.min_y();
    let height = proto.terrain.height();
    if min_y % 16 != 0 || height <= 0 || height % 16 != 0 {
        return Err(format!(
            "unexpected Horizon chunk bounds: {min_y} + {height}"
        ));
    }

    let mut states = Vec::with_capacity(height as usize * COLUMN_COUNT);
    let mut section = [0u32; SECTION_VOLUME];
    for section_y in (min_y..min_y + height).step_by(16) {
        if !proto.terrain.section_states(section_y, &mut section) {
            return Err(format!("Horizon chunk has no section at y={section_y}"));
        }
        states.extend_from_slice(&section);
    }

    let mut heights = [0i16; COLUMN_COUNT];
    for z in 0..16 {
        for x in 0..16 {
            heights[z * 16 + x] = proto.surface_height(x as i32, z as i32) as i16;
        }
    }

    let state_at = |x: i32, y: i32, z: i32| -> u32 {
        if !(0..16).contains(&x) || !(min_y..min_y + height).contains(&y) || !(0..16).contains(&z) {
            return 0;
        }
        let local_y = (y - min_y) as usize;
        states[(local_y << 8) | ((z as usize) << 4) | x as usize]
    };

    let mut vertices = Vec::with_capacity(24 * 1024);
    for y in min_y..min_y + height {
        for z in 0usize..16 {
            for x in 0usize..16 {
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

    Ok((vertices.into_boxed_slice(), heights))
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

fn generate_chunk_mesh(
    context: &mut WorldContext,
    chunk_x: i32,
    chunk_z: i32,
) -> Result<(Box<[TerrainVertex]>, [i16; COLUMN_COUNT]), String> {
    let position = horizon_core::position::ChunkPos::new(chunk_x, chunk_z);
    write_status(&format!("chunk generation started: {chunk_x},{chunk_z}\n"));
    let proto = context
        .generator
        .worker()
        .generate_proto(position)
        .map_err(|error| format!("Horizon chunk generation: {error}"))?;
    write_status("proto-chunk generated\n");
    mesh_chunk(&proto, &context.opaque, &context.materials)
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
    let (vertices, heights) = match generate_chunk_mesh(context, chunk_x, chunk_z) {
        Ok(mesh) => mesh,
        Err(error) => {
            copy_error(&error, error_out, error_capacity);
            return ptr::null_mut();
        }
    };
    let min_height = heights.iter().min().copied().unwrap_or_default();
    let max_height = heights.iter().max().copied().unwrap_or_default();
    write_status(&format!(
        "chunk meshed: {chunk_x},{chunk_z} vertices={} surface_y={min_height}..{max_height}\n",
        vertices.len()
    ));
    Box::into_raw(Box::new(WorldChunk {
        vertices,
        heights: Vec::from(heights).into_boxed_slice(),
        chunk_x,
        chunk_z,
    }))
    .cast::<c_void>()
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
    let mut vertices = Vec::new();
    let mut heights = vec![0i16; diameter * diameter];
    for chunk_z in origin_z..=end_z {
        for chunk_x in origin_x..=end_x {
            let (chunk_vertices, chunk_heights) =
                match generate_chunk_mesh(context, chunk_x, chunk_z) {
                    Ok(mesh) => mesh,
                    Err(error) => {
                        copy_error(&error, error_out, error_capacity);
                        return ptr::null_mut();
                    }
                };
            let local_chunk_x = ((chunk_x - origin_x) * 16) as usize;
            let local_chunk_z = ((chunk_z - origin_z) * 16) as usize;
            for z in 0..16 {
                let source_row = z * 16;
                let target_row = (local_chunk_z + z) * diameter + local_chunk_x;
                heights[target_row..target_row + 16]
                    .copy_from_slice(&chunk_heights[source_row..source_row + 16]);
            }
            let offset_x = local_chunk_x as f32;
            let offset_z = local_chunk_z as f32;
            let mut chunk_vertices = Vec::from(chunk_vertices);
            for vertex in &mut chunk_vertices {
                vertex.pos[0] += offset_x;
                vertex.pos[2] += offset_z;
            }
            vertices.append(&mut chunk_vertices);
        }
    }
    let min_height = heights.iter().min().copied().unwrap_or_default();
    let max_height = heights.iter().max().copied().unwrap_or_default();
    write_status(&format!(
        "region meshed: center={center_x},{center_z} chunks={} vertices={} surface_y={min_height}..{max_height}\n",
        side * side,
        vertices.len()
    ));
    Box::into_raw(Box::new(WorldChunk {
        vertices: vertices.into_boxed_slice(),
        heights: heights.into_boxed_slice(),
        chunk_x: origin_x,
        chunk_z: origin_z,
    }))
    .cast::<c_void>()
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
