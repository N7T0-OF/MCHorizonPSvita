#![no_std]

#[cfg(not(test))]
use core::panic::PanicInfo;
use horizon_core::position::BlockPos;
use horizon_terrain_data::TerrainVertex;

const SECTION_VERTEX_COUNT: u32 = 276;
#[cfg(test)]
const BLOCK_VERTEX_COUNT: u32 = SECTION_VERTEX_COUNT / 4 * 6;
const BASE_Y: i32 = 64;
const BLOCK_HEIGHTS: [i32; 9] = [1, 2, 1, 2, 3, 2, 1, 2, 1];
const TRIANGLE_VERTICES: [usize; 6] = [0, 1, 2, 0, 2, 3];
const FACE_UVS: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
const MAX_LIGHT_COORD: f32 = 240.0;

fn atlas_uv(tile_x: u32, tile_y: u32, uv: [f32; 2]) -> [f32; 2] {
    let tile_origin = [
        tile_x as f32 * 0.5 + 1.0 / 16.0,
        tile_y as f32 * 0.5 + 1.0 / 16.0,
    ];
    [
        tile_origin[0] + uv[0] * 3.0 / 8.0,
        tile_origin[1] + uv[1] * 3.0 / 8.0,
    ]
}

fn make_terrain_vertex(position: BlockPos, uv: [f32; 2], color: [f32; 3]) -> TerrainVertex {
    let y = (position.y - BASE_Y) as f32;
    let packed_color = 0xFF00_0000
        | ((color[2] * 255.0 + 0.5) as u32) << 16
        | ((color[1] * 255.0 + 0.5) as u32) << 8
        | (color[0] * 255.0 + 0.5) as u32;
    TerrainVertex::new(
        [position.x as f32 + 1.0, y, position.z as f32 + 1.0],
        uv,
        0,
        packed_color,
        0x00F0,
    )
}

/// Reduces Horizon's sky/block light coordinates to grayscale brightness.
///
/// Vanilla samples a colored, time-dependent lightmap. Until that texture and
/// its frame inputs are transferred, use the brighter channel with a small
/// ambient floor so dark sections remain visible.
fn terrain_brightness(light: u16) -> f32 {
    let sky = f32::from((light & 0x00FF).min(240)) / MAX_LIGHT_COORD;
    let block = f32::from((light >> 8).min(240)) / MAX_LIGHT_COORD;
    0.05 + 0.95 * sky.max(block)
}

fn write_gx2_vertex(
    vertices: &mut [f32],
    index: usize,
    terrain_vertex: TerrainVertex,
    section_origin: [f32; 3],
    camera_x: f32,
    camera_z: f32,
) {
    let x = section_origin[0] + terrain_vertex.pos[0] - camera_x;
    let y = section_origin[1] + terrain_vertex.pos[1];
    let z = section_origin[2] + terrain_vertex.pos[2] - camera_z;
    // Dimetric projection keeps the test world inside clip space and makes
    // horizontal GamePad movement easy to see without a matrix dependency.
    let projected = [
        (x - z) * 0.215,
        (x + z) * 0.10 - y * 0.25 + 0.40,
        0.5 + (x + z) * 0.015 - y * 0.025,
    ];
    let brightness = terrain_brightness(terrain_vertex.light);
    let color = terrain_vertex
        .rgba()
        .map(|component| f32::from(component) / 255.0 * brightness);
    let uv = terrain_vertex.uv();
    let base = index * 8;

    vertices[base..base + 3].copy_from_slice(&projected);
    vertices[base + 3..base + 6].copy_from_slice(&color[..3]);
    vertices[base + 6..base + 8].copy_from_slice(&uv);
}

fn append_face(
    vertices: &mut [TerrainVertex],
    first_vertex: usize,
    corners: [BlockPos; 4],
    tile: (u32, u32),
    color: [f32; 3],
) -> usize {
    for (corner_index, corner) in corners.into_iter().enumerate() {
        let uv = atlas_uv(tile.0, tile.1, FACE_UVS[corner_index]);
        vertices[first_vertex + corner_index] = make_terrain_vertex(corner, uv, color);
    }
    first_vertex + corners.len()
}

fn column_height(x: i32, z: i32) -> i32 {
    if !(-1..=1).contains(&x) || !(-1..=1).contains(&z) {
        return 0;
    }
    BLOCK_HEIGHTS[((z + 1) * 3 + (x + 1)) as usize]
}

fn append_column(
    vertices: &mut [TerrainVertex],
    first_vertex: usize,
    x: i32,
    z: i32,
    height: i32,
) -> usize {
    let mut next = first_vertex;

    for level in 0..height {
        let origin = BlockPos::new(x, BASE_Y + level, z);
        let x0y0z0 = origin;
        let x1y0z0 = origin.offset(1, 0, 0);
        let x0y0z1 = origin.offset(0, 0, 1);
        let x1y0z1 = origin.offset(1, 0, 1);
        let x0y1z0 = origin.offset(0, 1, 0);
        let x1y1z0 = origin.offset(1, 1, 0);
        let x0y1z1 = origin.offset(0, 1, 1);
        let x1y1z1 = origin.offset(1, 1, 1);

        if level == height - 1 {
            next = append_face(
                vertices,
                next,
                [x0y1z0, x1y1z0, x1y1z1, x0y1z1],
                (0, 0),
                [1.0, 1.0, 1.0],
            );
        }
        if level >= column_height(x + 1, z) {
            next = append_face(
                vertices,
                next,
                [x1y1z0, x1y0z0, x1y0z1, x1y1z1],
                (1, 0),
                [0.96, 0.96, 0.96],
            );
        }
        if level >= column_height(x, z + 1) {
            next = append_face(
                vertices,
                next,
                [x0y1z1, x1y1z1, x1y0z1, x0y0z1],
                (1, 0),
                [0.82, 0.82, 0.82],
            );
        }
        if level >= column_height(x - 1, z) {
            next = append_face(
                vertices,
                next,
                [x0y1z0, x0y1z1, x0y0z1, x0y0z0],
                (1, 0),
                [0.82, 0.82, 0.82],
            );
        }
        if level >= column_height(x, z - 1) {
            next = append_face(
                vertices,
                next,
                [x1y1z0, x0y1z0, x0y0z0, x1y0z0],
                (1, 0),
                [0.68, 0.68, 0.68],
            );
        }
    }
    next
}

/// Fills a small Horizon-positioned block landscape for the GX2 renderer.
///
/// The fixed 3x3 heightmap is an integration fixture. Each vertex is produced
/// from Horizon's real BlockPos type and then projected for GX2 clip space.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_build_block_section(
    vertices: *mut TerrainVertex,
    capacity: u32,
) -> u32 {
    if vertices.is_null() || capacity < SECTION_VERTEX_COUNT {
        return 0;
    }

    // SAFETY: C supplies an aligned writable array with the checked capacity.
    let vertices = unsafe { core::slice::from_raw_parts_mut(vertices, capacity as usize) };
    let mut next_vertex = 0;
    for (column, height) in BLOCK_HEIGHTS.into_iter().enumerate() {
        let x = column as i32 % 3 - 1;
        let z = column as i32 / 3 - 1;
        next_vertex = append_column(vertices, next_vertex, x, z, height);
    }

    next_vertex as u32
}

/// Converts Horizon's four-vertex terrain quads to GX2's triangle-list layout.
///
/// The source can be `SectionMesh::solid` in Horizon's 24-byte vertex format.
/// `section_x/y/z` is the section origin relative to the captured camera.
/// Sprite layers and light are retained in that format but are not sampled by
/// this bootstrap's single procedural texture shader yet.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_encode_terrain_quads(
    source: *const TerrainVertex,
    source_vertex_count: u32,
    output: *mut f32,
    output_capacity: u32,
    section_x: f32,
    section_y: f32,
    section_z: f32,
    camera_x: f32,
    camera_z: f32,
) -> u32 {
    if source.is_null() || output.is_null() || source_vertex_count == 0 {
        return 0;
    }
    if !source_vertex_count.is_multiple_of(4) {
        return 0;
    }

    let required_vertices = source_vertex_count / 4 * 6;
    if output_capacity < required_vertices {
        return 0;
    }
    let Some(output_scalar_count) = (output_capacity as usize).checked_mul(8) else {
        return 0;
    };

    // SAFETY: the C caller supplies a readable TerrainVertex array and a
    // writable GX2 float buffer matching the counts checked above.
    let source = unsafe { core::slice::from_raw_parts(source, source_vertex_count as usize) };
    let output = unsafe { core::slice::from_raw_parts_mut(output, output_scalar_count) };
    let section_origin = [section_x, section_y, section_z];
    let mut next_vertex = 0;
    for quad in source.chunks_exact(4) {
        for corner in TRIANGLE_VERTICES {
            write_gx2_vertex(
                output,
                next_vertex,
                quad[corner],
                section_origin,
                camera_x,
                camera_z,
            );
            next_vertex += 1;
        }
    }

    next_vertex as u32
}

/// Supplies the CafeOS frame clear color to the WUT presentation loop.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_frame_color(frame: u32, rgba: *mut f32) {
    if rgba.is_null() {
        return;
    }

    let phase = frame % 128;
    let ramp = if phase < 64 { phase } else { 127 - phase };
    let pulse = ramp as f32 / 63.0;

    // SAFETY: the C caller passes a pointer to a four-element RGBA array.
    unsafe {
        rgba.add(0).write(0.025 + pulse * 0.025);
        rgba.add(1).write(0.075 + pulse * 0.18);
        rgba.add(2).write(0.16 + pulse * 0.24);
        rgba.add(3).write(1.0);
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        horizon_wiiu_build_block_section, horizon_wiiu_encode_terrain_quads, terrain_brightness,
        BLOCK_VERTEX_COUNT, SECTION_VERTEX_COUNT,
    };
    use horizon_terrain_data::TerrainVertex;

    #[test]
    fn fixed_section_quads_expand_to_gx2_triangles() {
        let mut source =
            [TerrainVertex::new([0.0; 3], [0.0; 2], 0, 0, 0); SECTION_VERTEX_COUNT as usize];
        let source_count =
            horizon_wiiu_build_block_section(source.as_mut_ptr(), SECTION_VERTEX_COUNT);
        assert!(source_count > 0);
        assert_eq!(source_count % 4, 0);

        let mut output = [0.0f32; BLOCK_VERTEX_COUNT as usize * 8];
        let output_count = horizon_wiiu_encode_terrain_quads(
            source.as_ptr(),
            source_count,
            output.as_mut_ptr(),
            BLOCK_VERTEX_COUNT,
            -1.0,
            0.0,
            -1.0,
            0.0,
            0.0,
        );

        assert_eq!(output_count, source_count / 4 * 6);
        assert!(output_count <= BLOCK_VERTEX_COUNT);
        assert_eq!(source[0].pos, [0.0, 1.0, 0.0]);
        assert_eq!(source[0].rgba(), [255, 255, 255, 255]);
        assert_eq!(source[0].uv(), [1.0 / 16.0, 1.0 / 16.0]);
        assert_eq!(&output[3..6], &[1.0, 1.0, 1.0]);
        assert_eq!(output[6], 1.0 / 16.0);
        assert_eq!(output[7], 1.0 / 16.0);
        assert!((output[8] - 0.215).abs() < 0.00001);
    }

    #[test]
    fn captured_sky_and_block_light_control_vertex_brightness() {
        assert!((terrain_brightness(0) - 0.05).abs() < f32::EPSILON);
        assert!((terrain_brightness(0x00F0) - 1.0).abs() < f32::EPSILON);
        assert!((terrain_brightness(0x8000) - (0.05 + 0.95 * 128.0 / 240.0)).abs() < 0.00001);
        assert!((terrain_brightness(0x00F0 | 0x8000) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn quad_adapter_rejects_partial_quads_and_short_buffers() {
        let source = [TerrainVertex::new([0.0; 3], [0.0; 2], 0, 0, 0); 4];
        let mut output = [0.0f32; 6 * 8];
        assert_eq!(
            horizon_wiiu_encode_terrain_quads(
                source.as_ptr(),
                3,
                output.as_mut_ptr(),
                6,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            0
        );
        assert_eq!(
            horizon_wiiu_encode_terrain_quads(
                source.as_ptr(),
                4,
                output.as_mut_ptr(),
                5,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            0
        );
    }
}
