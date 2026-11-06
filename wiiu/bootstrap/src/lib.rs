#![no_std]

use core::panic::PanicInfo;
use core::ptr;
use horizon_core::position::BlockPos;

const BLOCK_VERTEX_COUNT: u32 = 18;

/// Fills a projected three-face block mesh using Horizon's vanilla BlockPos.
/// The mesh is deliberately a small GX2 smoke test, not Horizon world output.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_build_block_mesh(vertices: *mut f32, capacity: u32) -> u32 {
    if vertices.is_null() || capacity < BLOCK_VERTEX_COUNT {
        return 0;
    }

    let origin = BlockPos::new(0, 64, 0);
    let top = origin.offset(0, 1, 0);
    let x_top = origin.offset(1, 1, 0);
    let z_top = origin.offset(0, 1, 1);
    let far_top = origin.offset(1, 1, 1);
    let x_bottom = origin.offset(1, 0, 0);
    let z_bottom = origin.offset(0, 0, 1);
    let color_top = [0.56, 0.82, 0.34];
    let color_x = [0.34, 0.64, 0.30];
    let color_z = [0.22, 0.48, 0.26];

    // Each face is two triangles. The top and two visible sides form a simple
    // isometric block while keeping depth-buffer setup separate from this first
    // Horizon-data-to-GX2 vertex path.
    let mesh = [
        (top, color_top),
        (x_top, color_top),
        (far_top, color_top),
        (top, color_top),
        (far_top, color_top),
        (z_top, color_top),
        (top, color_x),
        (origin.offset(1, 1, 0), color_x),
        (x_bottom, color_x),
        (top, color_x),
        (x_bottom, color_x),
        (origin, color_x),
        (top, color_z),
        (z_top, color_z),
        (z_bottom, color_z),
        (top, color_z),
        (z_bottom, color_z),
        (origin, color_z),
    ];

    for (index, (position, color)) in mesh.into_iter().enumerate() {
        let x = position.x - origin.x;
        let y = position.y - origin.y;
        let z = position.z - origin.z;
        // Dimetric projection onto the bootstrap's existing clip-space shader.
        let projected = [
            (x - z) as f32 * 0.43,
            (x + z) as f32 * 0.20 - y as f32 * 0.50 + 0.05,
            0.0,
        ];
        let base = index * 6;
        // SAFETY: the caller promises a writable array of at least 18 vertices;
        // each vertex occupies six f32 values and capacity was checked above.
        unsafe {
            ptr::copy_nonoverlapping(projected.as_ptr(), vertices.add(base), 3);
            ptr::copy_nonoverlapping(color.as_ptr(), vertices.add(base + 3), 3);
        }
    }

    BLOCK_VERTEX_COUNT
}

/// Supplies the CafeOS frame clear color to the WUT presentation loop.
///
/// The cycling teal makes it clear that the WUT presentation loop is calling
/// Rust every frame while the block mesh remains static.
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
