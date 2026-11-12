//! Hardware 3D on the PS Vita's GXM.
//!
//! libvita2d owns the GXM context, the display buffers, the depth buffer and the
//! precompiled colour shader (`position * wvp`, per-vertex colour). This module feeds
//! that shader our own perspective matrix and our own GPU-mapped vertex and index
//! buffers, so no shader compiler is needed on the console.

use crate::terrain::{ChunkMesh, Vertex};
use core::ffi::{c_char, c_int, c_void};
use glam::{Mat4, Vec4};
use std::time::{Duration, Instant};
use vitasdk_sys::*;

#[allow(non_upper_case_globals)]
extern "C" {
    fn vita2d_init() -> c_int;
    fn vita2d_wait_rendering_done();
    fn vita2d_clear_screen();
    fn vita2d_swap_buffers();
    fn vita2d_start_drawing();
    fn vita2d_end_drawing();
    fn vita2d_set_clear_color(color: u32);
    fn vita2d_set_vblank_wait(enable: c_int);
    fn vita2d_get_context() -> *mut SceGxmContext;
    fn vita2d_load_default_pgf() -> *mut c_void;
    fn vita2d_draw_rectangle(x: f32, y: f32, w: f32, h: f32, color: u32);
    fn vita2d_pgf_draw_text(font: *mut c_void, x: c_int, y: c_int, color: u32, scale: f32, text: *const c_char) -> c_int;
    static _vita2d_colorVertexProgram: *mut SceGxmVertexProgram;
    static _vita2d_colorFragmentProgram: *mut SceGxmFragmentProgram;
    static _vita2d_colorWvpParam: *const SceGxmProgramParameter;
}

/// Memory the GPU can read: a kernel block mapped into GXM.
/// 8x8 fallback glyphs, used only when the system font cannot be loaded.
static FONT: &[u8; 2048] = include_bytes!("font.bin");

struct GpuBuffer {
    block: SceUID,
    base: *mut u8,
    bytes: usize,
}

impl GpuBuffer {
    fn new(bytes: usize) -> Result<Self, String> {
        let bytes = bytes.max(1).next_multiple_of(4096);
        let block = unsafe {
            sceKernelAllocMemBlock(c"horizon-mesh".as_ptr() as _, SCE_KERNEL_MEMBLOCK_TYPE_USER_RW as _, bytes as _, core::ptr::null_mut())
        };
        if block < 0 {
            return Err(format!("sceKernelAllocMemBlock({bytes}) = {block:#x}"));
        }
        let mut base: *mut c_void = core::ptr::null_mut();
        let got = unsafe { sceKernelGetMemBlockBase(block, &mut base) };
        if got < 0 || base.is_null() {
            unsafe { sceKernelFreeMemBlock(block) };
            return Err(format!("sceKernelGetMemBlockBase = {got:#x}"));
        }
        let mapped = unsafe { sceGxmMapMemory(base, bytes as _, SCE_GXM_MEMORY_ATTRIB_READ) };
        if mapped < 0 {
            unsafe { sceKernelFreeMemBlock(block) };
            return Err(format!("sceGxmMapMemory = {mapped:#x}"));
        }
        Ok(GpuBuffer { block, base: base as *mut u8, bytes })
    }
}

impl Drop for GpuBuffer {
    fn drop(&mut self) {
        unsafe {
            sceGxmUnmapMemory(self.base as *mut c_void);
            sceKernelFreeMemBlock(self.block);
        }
    }
}

struct ChunkDraw {
    vertex_offset: usize,
    index_offset: usize,
    index_count: u32,
    min: [f32; 3],
    max: [f32; 3],
}

pub struct Scene {
    vertices: GpuBuffer,
    indices: GpuBuffer,
    chunks: Vec<ChunkDraw>,
    pub vertex_count: usize,
    pub triangle_count: usize,
}

impl Scene {
    pub fn gpu_bytes(&self) -> usize {
        self.vertices.bytes + self.indices.bytes
    }
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

pub struct Drawn {
    pub chunks: usize,
    pub triangles: usize,
}

pub struct Renderer {
    context: *mut SceGxmContext,
    font: *mut c_void,
    text: Vec<u8>,
}

impl Renderer {
    pub fn new() -> Self {
        unsafe {
            vita2d_init();
            vita2d_set_clear_color(0xFFEB_B478); // sky, 0xAABBGGRR
        }
        Renderer { context: unsafe { vita2d_get_context() }, font: unsafe { vita2d_load_default_pgf() }, text: Vec::new() }
    }

    pub fn has_font(&self) -> bool {
        !self.font.is_null()
    }

    pub fn set_vsync(&mut self, on: bool) {
        unsafe { vita2d_set_vblank_wait(on as c_int) };
    }

    /// Copies the meshes into GPU-readable memory. Call `finish` first if a previous
    /// scene may still be drawing.
    pub fn upload(&mut self, meshes: &[ChunkMesh]) -> Result<Scene, String> {
        let vertex_count: usize = meshes.iter().map(|m| m.vertices.len()).sum();
        let index_count: usize = meshes.iter().map(|m| m.indices.len()).sum();
        let vertices = GpuBuffer::new(vertex_count * size_of::<Vertex>())?;
        let indices = GpuBuffer::new(index_count * size_of::<u16>())?;
        let mut chunks = Vec::with_capacity(meshes.len());
        let (mut vertex_offset, mut index_offset) = (0usize, 0usize);
        for mesh in meshes {
            let vertex_bytes = mesh.vertices.len() * size_of::<Vertex>();
            let index_bytes = mesh.indices.len() * size_of::<u16>();
            // SAFETY: both buffers were sized from the same sums, and are page aligned.
            unsafe {
                core::ptr::copy_nonoverlapping(mesh.vertices.as_ptr() as *const u8, vertices.base.add(vertex_offset), vertex_bytes);
                core::ptr::copy_nonoverlapping(mesh.indices.as_ptr() as *const u8, indices.base.add(index_offset), index_bytes);
            }
            if !mesh.indices.is_empty() {
                chunks.push(ChunkDraw { vertex_offset, index_offset, index_count: mesh.indices.len() as u32, min: mesh.min, max: mesh.max });
            }
            vertex_offset += vertex_bytes;
            index_offset += index_bytes;
        }
        Ok(Scene { vertices, indices, chunks, vertex_count, triangle_count: index_count / 3 })
    }

    pub fn begin(&mut self) {
        unsafe {
            vita2d_start_drawing();
            vita2d_clear_screen();
        }
    }

    /// Draws the chunks inside the view. `wvp` is projection * view, OpenGL conventions.
    pub fn draw(&mut self, scene: &Scene, wvp: Mat4, cull: bool) -> Drawn {
        let matrix = wvp.to_cols_array();
        let mut drawn = Drawn { chunks: 0, triangles: 0 };
        unsafe {
            let ctx = self.context;
            sceGxmSetFrontDepthFunc(ctx, SCE_GXM_DEPTH_FUNC_LESS_EQUAL);
            sceGxmSetBackDepthFunc(ctx, SCE_GXM_DEPTH_FUNC_LESS_EQUAL);
            sceGxmSetFrontDepthWriteEnable(ctx, SCE_GXM_DEPTH_WRITE_ENABLED);
            sceGxmSetBackDepthWriteEnable(ctx, SCE_GXM_DEPTH_WRITE_ENABLED);
            sceGxmSetCullMode(ctx, SCE_GXM_CULL_NONE);
            sceGxmSetVertexProgram(ctx, _vita2d_colorVertexProgram);
            sceGxmSetFragmentProgram(ctx, _vita2d_colorFragmentProgram);
            let mut uniforms: *mut c_void = core::ptr::null_mut();
            sceGxmReserveVertexDefaultUniformBuffer(ctx, &mut uniforms);
            sceGxmSetUniformDataF(uniforms, _vita2d_colorWvpParam, 0, 16, matrix.as_ptr());
            for chunk in &scene.chunks {
                if cull && !visible(&wvp, chunk.min, chunk.max) {
                    continue;
                }
                sceGxmSetVertexStream(ctx, 0, scene.vertices.base.add(chunk.vertex_offset) as *const c_void);
                sceGxmDraw(
                    ctx,
                    SCE_GXM_PRIMITIVE_TRIANGLES,
                    SCE_GXM_INDEX_FORMAT_U16,
                    scene.indices.base.add(chunk.index_offset) as *const c_void,
                    chunk.index_count,
                );
                drawn.chunks += 1;
                drawn.triangles += chunk.index_count as usize / 3;
            }
            // 2D drawn after this (the overlay) must not be hidden by the terrain's depth.
            sceGxmSetFrontDepthFunc(ctx, SCE_GXM_DEPTH_FUNC_ALWAYS);
            sceGxmSetBackDepthFunc(ctx, SCE_GXM_DEPTH_FUNC_ALWAYS);
        }
        drawn
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, colour: u32) {
        unsafe { vita2d_draw_rectangle(x as f32, y as f32, w as f32, h as f32, colour) };
    }

    /// Draws text with its baseline at `y`. `scale` 1.0 is about 17 pixels tall.
    pub fn text(&mut self, x: i32, y: i32, colour: u32, scale: f32, text: &str) {
        if self.font.is_null() {
            let pixel = (scale * 2.0).round().max(1.0) as i32;
            return self.bitmap_text(x, y - 7 * pixel, colour, pixel, text);
        }
        self.text.clear();
        self.text.extend(text.bytes().filter(|&b| b != 0));
        self.text.push(0);
        unsafe { vita2d_pgf_draw_text(self.font, x, y, colour, scale, self.text.as_ptr() as *const c_char) };
    }

    fn bitmap_text(&mut self, mut x: i32, y: i32, colour: u32, pixel: i32, text: &str) {
        for ch in text.chars() {
            let glyph = if ch.is_ascii() { ch as usize } else { b'?' as usize };
            for row in 0..8 {
                let bits = FONT[glyph * 8 + row];
                let mut col = 0;
                while col < 8 {
                    if bits & (0x80 >> col) == 0 {
                        col += 1;
                        continue;
                    }
                    let start = col;
                    while col < 8 && bits & (0x80 >> col) != 0 {
                        col += 1;
                    }
                    self.rect(x + start * pixel, y + row as i32 * pixel, (col - start) * pixel, pixel, colour);
                }
            }
            x += 8 * pixel;
        }
    }

    /// Ends the scene. With `time_gpu`, waits for the GPU and returns how long that took:
    /// a measure of the GPU's work, which costs the CPU/GPU overlap while it is on.
    pub fn end(&mut self, time_gpu: bool) -> Option<Duration> {
        unsafe { vita2d_end_drawing() };
        let waited = time_gpu.then(|| {
            let start = Instant::now();
            unsafe { vita2d_wait_rendering_done() };
            start.elapsed()
        });
        unsafe { vita2d_swap_buffers() };
        waited
    }

    /// Waits until the GPU no longer reads any buffer of ours.
    pub fn finish(&mut self) {
        unsafe { vita2d_wait_rendering_done() };
    }
}

/// False when the box is entirely outside one plane of the view volume.
fn visible(wvp: &Mat4, min: [f32; 3], max: [f32; 3]) -> bool {
    let mut outside = [0u8; 6];
    for corner in 0..8 {
        let point = Vec4::new(
            if corner & 1 == 0 { min[0] } else { max[0] },
            if corner & 2 == 0 { min[1] } else { max[1] },
            if corner & 4 == 0 { min[2] } else { max[2] },
            1.0,
        );
        let clip = *wvp * point;
        let tests = [clip.x < -clip.w, clip.x > clip.w, clip.y < -clip.w, clip.y > clip.w, clip.z < -clip.w, clip.z > clip.w];
        for (count, test) in outside.iter_mut().zip(tests) {
            *count += test as u8;
        }
    }
    outside.iter().all(|&count| count < 8)
}
