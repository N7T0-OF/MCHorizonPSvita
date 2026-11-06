use std::{ffi::CString, path::PathBuf};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let Some(resources) = args.next() else {
        eprintln!("usage: world_probe <prepared-26.3-resources> [seed] [overworld|flat] [radius]");
        std::process::exit(2);
    };
    let seed = args
        .next()
        .map(|value| {
            value
                .to_string_lossy()
                .parse::<i64>()
                .expect("seed is an integer")
        })
        .unwrap_or(12345);
    let profile = match args.next().as_deref().and_then(|value| value.to_str()) {
        None | Some("overworld") => 0,
        Some("flat") => 1,
        Some(other) => panic!("unknown profile {other:?}; use `overworld` or `flat`"),
    };
    let radius = args
        .next()
        .map(|value| {
            value
                .to_string_lossy()
                .parse::<u32>()
                .expect("region radius is an unsigned integer")
        })
        .unwrap_or(0);
    let resources = PathBuf::from(resources)
        .canonicalize()
        .expect("resource directory exists");
    let resources = CString::new(resources.to_string_lossy().as_bytes())
        .expect("resource path contains no nul byte");
    let mut spawn = [0i32; 2];
    let mut error = [0i8; 256];
    let initialized = horizon_wiiu_worldgen::horizon_wiiu_world_init(
        resources.as_ptr(),
        seed,
        profile,
        spawn.as_mut_ptr(),
        error.as_mut_ptr(),
        error.len() as u32,
    );
    assert_eq!(initialized, 1, "world init failed: {}", unsafe {
        std::ffi::CStr::from_ptr(error.as_ptr()).to_string_lossy()
    });
    let chunk_x = spawn[0].div_euclid(16);
    let chunk_z = spawn[1].div_euclid(16);
    let chunk = if radius == 0 {
        horizon_wiiu_worldgen::horizon_wiiu_world_generate_chunk(
            chunk_x,
            chunk_z,
            error.as_mut_ptr(),
            error.len() as u32,
        )
    } else {
        horizon_wiiu_worldgen::horizon_wiiu_world_generate_region(
            chunk_x,
            chunk_z,
            radius,
            error.as_mut_ptr(),
            error.len() as u32,
        )
    };
    assert!(!chunk.is_null(), "chunk generation failed: {}", unsafe {
        std::ffi::CStr::from_ptr(error.as_ptr()).to_string_lossy()
    });
    let mut vertex_count = 0;
    let vertices =
        horizon_wiiu_worldgen::horizon_wiiu_world_chunk_vertices(chunk, &mut vertex_count);
    assert!(!vertices.is_null());
    let height_diameter = ((radius * 2 + 1) * 16) as usize;
    let mut heights = vec![0i16; height_diameter * height_diameter];
    assert_eq!(
        horizon_wiiu_worldgen::horizon_wiiu_world_chunk_heights(
            chunk,
            heights.as_mut_ptr(),
            heights.len() as u32,
        ),
        1
    );
    let min_height = heights.iter().min().copied().unwrap_or_default();
    let max_height = heights.iter().max().copied().unwrap_or_default();
    let has_local_atlas = !horizon_wiiu_worldgen::horizon_wiiu_world_terrain_atlas().is_null();
    let mut timings = [0u64; 6];
    assert_eq!(
        horizon_wiiu_worldgen::horizon_wiiu_world_stage_timings(timings.as_mut_ptr()),
        1
    );
    let mut mesh_origin = [0i32; 2];
    assert_eq!(
        horizon_wiiu_worldgen::horizon_wiiu_world_chunk_position(chunk, mesh_origin.as_mut_ptr()),
        1
    );
    if profile == 1 && radius > 0 {
        let local_x = spawn[0] - mesh_origin[0] * 16;
        let local_z = spawn[1] - mesh_origin[1] * 16;
        let height_index = local_z as usize * height_diameter + local_x as usize;
        let original_height = heights[height_index];
        let origin = [
            spawn[0] as f32 + 0.5,
            f32::from(original_height) + 1.62,
            spawn[1] as f32 + 0.5,
        ];
        let direction = [0.0f32, -1.0, 0.0];
        println!(
            "interaction target probe: spawn=({}, {}) region_origin=({}, {}) height={} ray_origin={origin:?}",
            spawn[0], spawn[1], mesh_origin[0], mesh_origin[1], original_height
        );
        assert_eq!(
            horizon_wiiu_worldgen::horizon_wiiu_world_player_clear(
                chunk, origin[0], origin[1], origin[2], 0.30, 1.80,
            ),
            1,
            "standing on the Horizon surface should leave the player AABB clear"
        );
        assert_eq!(
            horizon_wiiu_worldgen::horizon_wiiu_world_player_clear(
                chunk,
                origin[0],
                origin[1] - 1.0,
                origin[2],
                0.30,
                1.80,
            ),
            0,
            "placing the player inside the top block should be blocked"
        );
        let mut hit = [0i32; 6];
        assert_eq!(
            horizon_wiiu_worldgen::horizon_wiiu_world_raycast(
                chunk,
                origin.as_ptr(),
                direction.as_ptr(),
                5.0,
                hit.as_mut_ptr(),
            ),
            1,
            "spawn column should be targetable"
        );
        assert_eq!(
            hit,
            [
                spawn[0],
                i32::from(original_height) - 1,
                spawn[1],
                spawn[0],
                i32::from(original_height),
                spawn[1]
            ]
        );

        assert_eq!(
            horizon_wiiu_worldgen::horizon_wiiu_world_set_block(chunk, hit[0], hit[1], hit[2], 0),
            1,
            "breaking the targeted grass block should edit Horizon block state"
        );
        let mut edited_heights = vec![0i16; height_diameter * height_diameter];
        assert_eq!(
            horizon_wiiu_worldgen::horizon_wiiu_world_chunk_heights(
                chunk,
                edited_heights.as_mut_ptr(),
                edited_heights.len() as u32,
            ),
            1
        );
        assert_eq!(edited_heights[height_index], original_height - 1);
        let placement_state = horizon_wiiu_worldgen::horizon_wiiu_world_placement_state();
        assert_ne!(placement_state, 0);
        assert_eq!(
            horizon_wiiu_worldgen::horizon_wiiu_world_set_block(
                chunk,
                hit[0],
                hit[1],
                hit[2],
                placement_state
            ),
            1,
            "placing the prototype dirt state should update the same block"
        );
        let mut restored_heights = vec![0i16; height_diameter * height_diameter];
        assert_eq!(
            horizon_wiiu_worldgen::horizon_wiiu_world_chunk_heights(
                chunk,
                restored_heights.as_mut_ptr(),
                restored_heights.len() as u32,
            ),
            1
        );
        assert_eq!(restored_heights[height_index], original_height);

        let reloaded = horizon_wiiu_worldgen::horizon_wiiu_world_generate_region(
            chunk_x,
            chunk_z,
            radius,
            error.as_mut_ptr(),
            error.len() as u32,
        );
        assert!(
            !reloaded.is_null(),
            "edited region should reload: {}",
            unsafe { std::ffi::CStr::from_ptr(error.as_ptr()).to_string_lossy() }
        );
        let mut persisted_heights = vec![0i16; height_diameter * height_diameter];
        assert_eq!(
            horizon_wiiu_worldgen::horizon_wiiu_world_chunk_heights(
                reloaded,
                persisted_heights.as_mut_ptr(),
                persisted_heights.len() as u32,
            ),
            1
        );
        assert_eq!(persisted_heights[height_index], original_height);
        horizon_wiiu_worldgen::horizon_wiiu_world_free_chunk(reloaded);
        println!(
            "live interaction probe: raycast, break, place and region-reload persistence passed"
        );
    }
    println!(
        "Horizon 26.3 profile={profile} seed={seed} spawn=({}, {}) chunk=({}, {}) region_origin=({}, {}) radius={radius} mesh_vertices={vertex_count} surface_y={min_height}..{max_height} atlas={} generated_chunks={} stage_ms=biomes:{},noise:{},surface:{},structures:{},carvers:{}",
        spawn[0], spawn[1], chunk_x, chunk_z, mesh_origin[0], mesh_origin[1],
        if has_local_atlas { "local" } else { "placeholder" },
        timings[0],
        timings[1] / 1000,
        timings[2] / 1000,
        timings[3] / 1000,
        timings[4] / 1000,
        timings[5] / 1000
    );
    horizon_wiiu_worldgen::horizon_wiiu_world_free_chunk(chunk);
}
