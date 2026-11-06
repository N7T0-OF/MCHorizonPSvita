use std::{ffi::CString, path::PathBuf};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let Some(resources) = args.next() else {
        eprintln!("usage: world_probe <prepared-26.3-resources> [seed]");
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
    let chunk = horizon_wiiu_worldgen::horizon_wiiu_world_generate_chunk(
        chunk_x,
        chunk_z,
        error.as_mut_ptr(),
        error.len() as u32,
    );
    assert!(!chunk.is_null(), "chunk generation failed: {}", unsafe {
        std::ffi::CStr::from_ptr(error.as_ptr()).to_string_lossy()
    });
    let mut vertex_count = 0;
    let vertices =
        horizon_wiiu_worldgen::horizon_wiiu_world_chunk_vertices(chunk, &mut vertex_count);
    assert!(!vertices.is_null());
    let mut heights = [0i16; 256];
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
    println!(
        "Horizon 26.3 profile={profile} seed={seed} spawn=({}, {}) chunk=({}, {}) mesh_vertices={vertex_count} surface_y={min_height}..{max_height}",
        spawn[0], spawn[1], chunk_x, chunk_z
    );
    horizon_wiiu_worldgen::horizon_wiiu_world_free_chunk(chunk);
}
