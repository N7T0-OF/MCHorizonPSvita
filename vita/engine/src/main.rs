//! Horizon Vita 3D bring-up. It is NOT the game: it proves hardware 3D on the PS Vita
//! (perspective, depth, chunked meshes, stick camera) and measures what it costs.
//!
//! On the PC (`cargo run --release`) it builds the same world and prints the numbers
//! the console must match.

mod terrain;

#[cfg(target_os = "vita")]
mod gfx;

use terrain::{World, CHUNKS_PER_SIDE};

pub const VERSION: &str = "0.0.1";
const FIRST_SEED: i64 = 20261112;

/// Counts what the Rust heap holds.
mod heap {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    pub static LIVE: AtomicUsize = AtomicUsize::new(0);
    pub static PEAK: AtomicUsize = AtomicUsize::new(0);

    pub struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                let live = LIVE.fetch_add(layout.size(), Relaxed) + layout.size();
                PEAK.fetch_max(live, Relaxed);
            }
            ptr
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) };
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
    }
}

#[global_allocator]
static HEAP: heap::Counting = heap::Counting;

pub struct Built {
    pub world: World,
    pub meshes: Vec<terrain::ChunkMesh>,
    pub generate: std::time::Duration,
    pub mesh: std::time::Duration,
}

pub fn build(seed: i64) -> Built {
    let start = std::time::Instant::now();
    let world = World::new(seed);
    let generate = start.elapsed();
    let start = std::time::Instant::now();
    let mut meshes = Vec::with_capacity(CHUNKS_PER_SIDE * CHUNKS_PER_SIDE);
    for cz in 0..CHUNKS_PER_SIDE {
        for cx in 0..CHUNKS_PER_SIDE {
            meshes.push(world.mesh_chunk(cx, cz));
        }
    }
    Built { world, meshes, generate, mesh: start.elapsed() }
}

#[cfg(not(target_os = "vita"))]
fn main() {
    let built = build(FIRST_SEED);
    let vertices: usize = built.meshes.iter().map(|m| m.vertices.len()).sum();
    let indices: usize = built.meshes.iter().map(|m| m.indices.len()).sum();
    println!(
        "seed {FIRST_SEED}: checksum {:08X}, {} chunks, {vertices} vertices, {} triangles, generate {:?}, mesh {:?}, heap peak {} bytes",
        built.world.checksum(),
        built.meshes.len(),
        indices / 3,
        built.generate,
        built.mesh,
        heap::PEAK.load(std::sync::atomic::Ordering::Relaxed)
    );
}

#[cfg(target_os = "vita")]
fn main() {
    vita::run();
}

#[cfg(target_os = "vita")]
mod vita {
    use super::{build, gfx, heap, terrain, FIRST_SEED, VERSION};
    use glam::{Mat4, Vec3};
    use std::io::Write as _;
    use std::sync::atomic::Ordering::Relaxed;
    use std::time::Instant;
    use vitasdk_sys::*;

    const LOG: &str = "ux0:data/HorizonVita/engine.log";
    const WHITE: u32 = 0xFFFF_FFFF;
    const YELLOW: u32 = 0xFF5B_E0FF;
    /// Frames averaged for the numbers on screen and in the log.
    const WINDOW: u32 = 30;

    struct Log(Option<std::fs::File>, u32);

    impl Log {
        fn line(&mut self, text: &str) {
            if let Some(file) = &mut self.0 {
                let _ = writeln!(file, "{text}");
                let _ = file.flush();
            }
        }
        /// Periodic lines, capped so a long session cannot fill the card.
        fn sample(&mut self, text: &str) {
            if self.1 < 400 {
                self.1 += 1;
                self.line(text);
            }
        }
    }

    fn free_memory() -> (i32, i32) {
        let mut info: SceKernelFreeMemorySizeInfo = unsafe { core::mem::zeroed() };
        info.size = core::mem::size_of::<SceKernelFreeMemorySizeInfo>() as _;
        if unsafe { sceKernelGetFreeMemorySize(&mut info) } < 0 {
            return (-1, -1);
        }
        (info.size_user as i32 / 1024, info.size_cdram as i32 / 1024)
    }

    fn axis(raw: u8) -> f32 {
        let value = (raw as f32 - 128.0) / 127.0;
        if value.abs() < 0.2 { 0.0 } else { value.clamp(-1.0, 1.0) }
    }

    fn load(renderer: &mut gfx::Renderer, log: &mut Log, seed: i64) -> Option<(gfx::Scene, terrain::World, String)> {
        let built = build(seed);
        let start = Instant::now();
        let scene = match renderer.upload(&built.meshes) {
            Ok(scene) => scene,
            Err(error) => {
                log.line(&format!("upload failed: {error}"));
                return None;
            }
        };
        let summary = format!(
            "seed {seed} checksum {:08X}: {} chunks, {} vertices, {} triangles, {} KiB GPU-mapped; generate {} ms, mesh {} ms, upload {} ms",
            built.world.checksum(),
            scene.chunk_count(),
            scene.vertex_count,
            scene.triangle_count,
            scene.gpu_bytes() / 1024,
            built.generate.as_millis(),
            built.mesh.as_millis(),
            start.elapsed().as_millis()
        );
        log.line(&summary);
        Some((scene, built.world, summary))
    }

    pub fn run() {
        let _ = std::fs::create_dir_all("ux0:data/HorizonVita");
        let mut log = Log(std::fs::File::create(LOG).ok(), 0);
        std::panic::set_hook(Box::new(|info| {
            let _ = std::fs::write("ux0:data/HorizonVita/engine-panic.txt", format!("{info}\n"));
        }));
        log.line(&format!("Horizon Vita 3D DEV {VERSION} - GXM renderer bring-up, not the game"));
        let (user, cdram) = free_memory();
        log.line(&format!("free before graphics: user {user} KiB, cdram {cdram} KiB"));

        let mut renderer = gfx::Renderer::new();
        log.line(&format!("vita2d/GXM initialised, system font {}", if renderer.has_font() { "loaded" } else { "MISSING: no text on screen" }));
        unsafe { sceCtrlSetSamplingMode(SCE_CTRL_MODE_ANALOG) };

        let mut seed = FIRST_SEED;
        let Some((mut scene, mut world, mut summary)) = load(&mut renderer, &mut log, seed) else {
            return;
        };
        let (user, cdram) = free_memory();
        log.line(&format!("free after first scene: user {user} KiB, cdram {cdram} KiB, heap peak {} B", heap::PEAK.load(Relaxed)));

        let side = terrain::SIDE as f32;
        let mut position = Vec3::new(side / 2.0, 52.0, side - 6.0);
        let (mut yaw, mut pitch) = (0.0f32, -0.45f32);
        let (mut vsync, mut cull, mut time_gpu) = (true, true, true);
        let mut previous = u32::MAX;
        let mut last = Instant::now();
        // Sums over the current window, and the last finished window's averages.
        let (mut frames, mut sum_frame, mut sum_cpu, mut sum_gpu) = (0u32, 0.0f32, 0.0f32, 0.0f32);
        let (mut fps, mut frame_ms, mut cpu_ms, mut gpu_ms) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let mut line = String::new();

        loop {
            let now = Instant::now();
            let dt = (now - last).as_secs_f32().min(0.1);
            last = now;

            let mut pad: SceCtrlData = unsafe { core::mem::zeroed() };
            if unsafe { sceCtrlPeekBufferPositive(0, &mut pad, 1) } > 0 {
                let pressed = pad.buttons & !previous;
                previous = pad.buttons;
                if pressed & SCE_CTRL_START != 0 {
                    break;
                }
                if pressed & SCE_CTRL_TRIANGLE != 0 {
                    vsync = !vsync;
                    renderer.set_vsync(vsync);
                }
                if pressed & SCE_CTRL_SQUARE != 0 {
                    cull = !cull;
                }
                if pressed & SCE_CTRL_CIRCLE != 0 {
                    time_gpu = !time_gpu;
                }
                if pressed & SCE_CTRL_CROSS != 0 {
                    seed += 1;
                    renderer.finish();
                    if let Some(next) = load(&mut renderer, &mut log, seed) {
                        (scene, world, summary) = next;
                    }
                }
                yaw += axis(pad.rx) * 2.2 * dt;
                pitch = (pitch - axis(pad.ry) * 1.8 * dt).clamp(-1.5, 1.5);
                let forward = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
                let right = Vec3::new(yaw.cos(), 0.0, yaw.sin());
                let lift = (pad.buttons & SCE_CTRL_RTRIGGER != 0) as i32 - (pad.buttons & SCE_CTRL_LTRIGGER != 0) as i32;
                position += (forward * -axis(pad.ly) + right * axis(pad.lx) + Vec3::Y * lift as f32) * 16.0 * dt;
            }
            // Stay above the ground: this is a fly camera, not player physics.
            if position.x >= 0.0 && position.z >= 0.0 && position.x < side && position.z < side {
                position.y = position.y.max(world.surface_at(position.x, position.z) + 1.7);
            }

            let forward = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
            let wvp = Mat4::perspective_rh_gl(70f32.to_radians(), 960.0 / 544.0, 0.1, 400.0) * Mat4::look_to_rh(position, forward, Vec3::Y);

            renderer.begin();
            let drawn = renderer.draw(&scene, wvp, cull);

            let (user, cdram) = free_memory();
            line.clear();
            line.push_str("Horizon Vita 3D DEV ");
            line.push_str(VERSION);
            line.push_str(" - renderer test, NOT the game");
            renderer.text(16, 26, YELLOW, &line);
            renderer.text(
                16,
                48,
                WHITE,
                &format!(
                    "{fps:.1} FPS   frame {frame_ms:.1} ms   cpu {cpu_ms:.1} ms   gpu {}",
                    if time_gpu { format!("{gpu_ms:.1} ms") } else { "off".into() }
                ),
            );
            renderer.text(
                16,
                70,
                WHITE,
                &format!(
                    "chunks {}/{}   triangles {}   vsync {}   culling {}",
                    drawn.chunks,
                    scene.chunk_count(),
                    drawn.triangles,
                    if vsync { "on" } else { "off" },
                    if cull { "on" } else { "off" }
                ),
            );
            renderer.text(16, 92, WHITE, &format!("free RAM {user} KiB   free VRAM {cdram} KiB   heap {} KiB", heap::LIVE.load(Relaxed) / 1024));
            renderer.text(16, 114, WHITE, &summary[..summary.find(':').unwrap_or(summary.len())]);
            renderer.text(16, 506, WHITE, "sticks: move/look   L/R: down/up   CROSS: next seed   START: exit");
            renderer.text(16, 528, WHITE, "TRIANGLE: vsync   SQUARE: culling   CIRCLE: gpu timing");

            let cpu = now.elapsed();
            let gpu = renderer.end(time_gpu);

            frames += 1;
            sum_frame += dt * 1000.0;
            sum_cpu += cpu.as_secs_f32() * 1000.0;
            sum_gpu += gpu.map_or(0.0, |g| g.as_secs_f32() * 1000.0);
            if frames == WINDOW {
                frame_ms = sum_frame / WINDOW as f32;
                cpu_ms = sum_cpu / WINDOW as f32;
                gpu_ms = sum_gpu / WINDOW as f32;
                fps = 1000.0 / frame_ms.max(0.001);
                (frames, sum_frame, sum_cpu, sum_gpu) = (0, 0.0, 0.0, 0.0);
                log.sample(&format!(
                    "fps {fps:.1} frame {frame_ms:.2} cpu {cpu_ms:.2} gpu {} chunks {}/{} tris {} vsync {vsync} cull {cull} free {user}/{cdram} KiB pos {:.0},{:.0},{:.0}",
                    if time_gpu { format!("{gpu_ms:.2}") } else { "off".into() },
                    drawn.chunks,
                    scene.chunk_count(),
                    drawn.triangles,
                    position.x,
                    position.y,
                    position.z
                ));
            }
        }
        renderer.finish();
        log.line("clean exit");
    }
}
