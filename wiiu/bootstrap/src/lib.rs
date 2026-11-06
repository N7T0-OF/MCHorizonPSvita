#![no_std]

use core::panic::PanicInfo;
use core::ptr;
use horizon_core::position::BlockPos;
use horizon_terrain_data::TerrainVertex;

const BLOCK_VERTEX_COUNT: u32 = 414;
const BASE_Y: i32 = 64;
const BLOCK_HEIGHTS: [i32; 9] = [1, 2, 1, 2, 3, 2, 1, 2, 1];
const TRIANGLE_VERTICES: [usize; 6] = [0, 1, 2, 0, 2, 3];
const FACE_UVS: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

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

fn write_vertex(
    vertices: *mut f32,
    index: usize,
    position: BlockPos,
    uv: [f32; 2],
    color: [f32; 3],
    camera_x: f32,
    camera_z: f32,
) {
    let y = (position.y - BASE_Y) as f32;
    let packed_color = 0xFF00_0000
        | ((color[2] * 255.0 + 0.5) as u32) << 16
        | ((color[1] * 255.0 + 0.5) as u32) << 8
        | (color[0] * 255.0 + 0.5) as u32;
    let terrain_vertex = TerrainVertex::new(
        [position.x as f32 + 1.0, y, position.z as f32 + 1.0],
        uv,
        0,
        packed_color,
        0,
    );
    let x = terrain_vertex.pos[0] - 1.0 - camera_x;
    let y = terrain_vertex.pos[1];
    let z = terrain_vertex.pos[2] - 1.0 - camera_z;
    // Dimetric projection keeps the test world inside clip space and makes
    // horizontal GamePad movement easy to see without a matrix dependency.
    let projected = [
        (x - z) * 0.215,
        (x + z) * 0.10 - y * 0.25 + 0.40,
        0.5 + (x + z) * 0.015 - y * 0.025,
    ];
    let color = terrain_vertex
        .rgba()
        .map(|component| f32::from(component) / 255.0);
    let uv = terrain_vertex.uv();
    let base = index * 8;

    // SAFETY: the exported mesh builder validates capacity before writing;
    // the caller provides space for the fixed heightmap's 414-vertex maximum.
    unsafe {
        ptr::copy_nonoverlapping(projected.as_ptr(), vertices.add(base), 3);
        ptr::copy_nonoverlapping(color.as_ptr(), vertices.add(base + 3), 3);
        ptr::copy_nonoverlapping(uv.as_ptr(), vertices.add(base + 6), 2);
    }
}

fn append_face(
    vertices: *mut f32,
    first_vertex: usize,
    corners: [BlockPos; 4],
    tile: (u32, u32),
    color: [f32; 3],
    camera_x: f32,
    camera_z: f32,
) -> usize {
    for (vertex_offset, corner_index) in TRIANGLE_VERTICES.into_iter().enumerate() {
        let uv = atlas_uv(tile.0, tile.1, FACE_UVS[corner_index]);
        write_vertex(
            vertices,
            first_vertex + vertex_offset,
            corners[corner_index],
            uv,
            color,
            camera_x,
            camera_z,
        );
    }
    first_vertex + TRIANGLE_VERTICES.len()
}

fn column_height(x: i32, z: i32) -> i32 {
    if !(-1..=1).contains(&x) || !(-1..=1).contains(&z) {
        return 0;
    }
    BLOCK_HEIGHTS[((z + 1) * 3 + (x + 1)) as usize]
}

fn append_column(
    vertices: *mut f32,
    first_vertex: usize,
    x: i32,
    z: i32,
    height: i32,
    camera_x: f32,
    camera_z: f32,
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
                camera_x,
                camera_z,
            );
        }
        if level >= column_height(x + 1, z) {
            next = append_face(
                vertices,
                next,
                [x1y1z0, x1y0z0, x1y0z1, x1y1z1],
                (1, 0),
                [0.96, 0.96, 0.96],
                camera_x,
                camera_z,
            );
        }
        if level >= column_height(x, z + 1) {
            next = append_face(
                vertices,
                next,
                [x0y1z1, x1y1z1, x1y0z1, x0y0z1],
                (1, 0),
                [0.82, 0.82, 0.82],
                camera_x,
                camera_z,
            );
        }
        if level >= column_height(x - 1, z) {
            next = append_face(
                vertices,
                next,
                [x0y1z0, x0y1z1, x0y0z1, x0y0z0],
                (1, 0),
                [0.82, 0.82, 0.82],
                camera_x,
                camera_z,
            );
        }
        if level >= column_height(x, z - 1) {
            next = append_face(
                vertices,
                next,
                [x1y1z0, x0y1z0, x0y0z0, x1y0z0],
                (1, 0),
                [0.68, 0.68, 0.68],
                camera_x,
                camera_z,
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
pub extern "C" fn horizon_wiiu_build_block_mesh(
    vertices: *mut f32,
    capacity: u32,
    camera_x: f32,
    camera_z: f32,
) -> u32 {
    if vertices.is_null() || capacity < BLOCK_VERTEX_COUNT {
        return 0;
    }

    let mut next_vertex = 0;
    for (column, height) in BLOCK_HEIGHTS.into_iter().enumerate() {
        let x = column as i32 % 3 - 1;
        let z = column as i32 / 3 - 1;
        next_vertex = append_column(vertices, next_vertex, x, z, height, camera_x, camera_z);
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

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
