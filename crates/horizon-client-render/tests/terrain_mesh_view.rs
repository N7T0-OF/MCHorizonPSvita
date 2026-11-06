use horizon_client_render::chunk::{Chunk, Light, MIN_Y};
use horizon_terrain_mesh::{MeshChunk, MeshSectionView, block_index};
use std::sync::Arc;

#[test]
fn mesh_view_borrows_the_canonical_section_and_reads_its_data() {
    let (x, y, z) = (3, MIN_Y + 8, 11);
    let mut chunk = Chunk::empty(-2, 5);
    chunk.set(x, y, z, 37);
    let section_index = ((y - MIN_Y) >> 4) as usize;
    let light_at = block_index(x, y, z);
    let mut light = Box::new([0; 4096]);
    light[light_at] = 0xA5;
    Arc::make_mut(&mut chunk.sections[section_index]).light = Light::Full(light);
    chunk.set_biome_cell(0, 2, 2, 19);

    let section = chunk.section_at(y).unwrap();
    let view = MeshChunk::section_at(&chunk, y).unwrap();
    assert_eq!(
        section as *const _ as *const (), view as *const dyn MeshSectionView as *const (),
        "MeshChunk::section_at must borrow the canonical Section"
    );
    let mut blocks = [0; 4096];
    view.unpack(&mut blocks);
    assert_eq!(blocks[light_at], 37);
    let mut run = [0; 2];
    view.get_run(light_at, &mut run);
    assert_eq!(run, [37, 0]);
    let mut lights = [0; 2];
    view.copy_light_run(light_at, &mut lights);
    assert_eq!(lights, [0xA5, 0]);
    assert!(!view.is_uniform_air());
    assert_eq!(MeshChunk::x(&chunk), -2);
    assert_eq!(MeshChunk::z(&chunk), 5);
    assert_eq!(MeshChunk::get(&chunk, x, y, z), 37);
    assert_eq!(MeshChunk::light(&chunk, x, y, z), 0xA5);
    assert_eq!(MeshChunk::biome(&chunk, 0, y, 8), 19);
    assert_eq!(
        <Chunk as MeshChunk>::section_at(&chunk, MIN_Y - 1).err(),
        Some(0)
    );
    assert_eq!(
        <Chunk as MeshChunk>::section_at(&chunk, chunk.max_y()).err(),
        Some(0xF0)
    );
}
