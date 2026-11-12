//! The application on the console: menu, mod folders, diagnostics and the 3D test,
//! as screens of one frame loop.

use crate::{build, gfx, heap, terrain, EXPECTED_NOISE, EXPECTED_WORLD, FIRST_SEED, VERSION};
use glam::{Mat4, Vec3};
use std::io::Write as _;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;
use vitasdk_sys::*;

const DATA: &str = "ux0:data/HorizonVita";
/// (label, folder under `DATA`). Nothing here is loaded yet: there is no game engine.
const FOLDERS: [(&str, &str); 4] = [("Mods", "mods"), ("Packs de ressources", "resourcepacks"), ("Shaders", "shaderpacks"), ("Sauvegardes", "saves")];
/// Frames averaged for the numbers shown and logged by the 3D test.
const WINDOW: u32 = 30;

const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    0xFF00_0000 | b << 16 | g << 8 | r
}
const BACKGROUND: u32 = rgb(13, 18, 25);
const HEADER: u32 = rgb(21, 32, 40);
const TEAL: u32 = rgb(46, 197, 169);
const PANEL: u32 = rgb(26, 39, 51);
const SELECTED: u32 = rgb(34, 77, 76);
const TEXT: u32 = rgb(238, 246, 245);
const DIM: u32 = rgb(151, 168, 179);
const WARN: u32 = rgb(255, 182, 91);
const BAD: u32 = rgb(255, 120, 120);
const GOOD: u32 = rgb(120, 230, 150);

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

/// Free memory in KiB: (user RAM, video memory), or -1 when the query fails.
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

/// Creates the user folders and counts what each holds (`None`: cannot be read).
fn scan_folders(log: &mut Log) -> [Option<usize>; 4] {
    let mut counts = [None; 4];
    for (count, (_, folder)) in counts.iter_mut().zip(FOLDERS) {
        let path = format!("{DATA}/{folder}");
        let _ = std::fs::create_dir_all(&path);
        *count = std::fs::read_dir(&path).ok().map(|entries| entries.count());
        log.line(&format!("folder {path}: {count:?}"));
    }
    counts
}

/// The 3D test: a scene on the GPU and a fly camera.
struct Game {
    scene: gfx::Scene,
    world: terrain::World,
    summary: String,
    seed: i64,
    position: Vec3,
    yaw: f32,
    pitch: f32,
    vsync: bool,
    cull: bool,
    time_gpu: bool,
    frames: u32,
    sums: [f32; 3],
    /// Averages of the last finished window: frame, cpu, gpu milliseconds.
    shown: [f32; 3],
}

impl Game {
    fn load(renderer: &mut gfx::Renderer, log: &mut Log, seed: i64) -> Result<(gfx::Scene, terrain::World, String), String> {
        let built = build(seed);
        let start = Instant::now();
        let scene = renderer.upload(&built.meshes)?;
        let checksum = built.world.checksum();
        let summary = format!(
            "graine {seed}  checksum {checksum:08X}{}",
            if seed != FIRST_SEED {
                ""
            } else if checksum == EXPECTED_WORLD {
                "  (identique au PC)"
            } else {
                "  (DIFFÉRENT du PC)"
            }
        );
        log.line(&format!(
            "scene seed {seed} checksum {checksum:08X}: {} chunks, {} vertices, {} triangles, {} KiB GPU-mapped; generate {} ms, mesh {} ms, upload {} ms",
            scene.chunk_count(),
            scene.vertex_count,
            scene.triangle_count,
            scene.gpu_bytes() / 1024,
            built.generate.as_millis(),
            built.mesh.as_millis(),
            start.elapsed().as_millis()
        ));
        Ok((scene, built.world, summary))
    }

    fn new(renderer: &mut gfx::Renderer, log: &mut Log) -> Result<Self, String> {
        let (scene, world, summary) = Self::load(renderer, log, FIRST_SEED)?;
        let (user, cdram) = free_memory();
        log.line(&format!("free after scene: user {user} KiB, cdram {cdram} KiB, heap peak {} B", heap::PEAK.load(Relaxed)));
        let side = terrain::SIDE as f32;
        Ok(Game {
            scene,
            world,
            summary,
            seed: FIRST_SEED,
            position: Vec3::new(side / 2.0, 52.0, side - 6.0),
            yaw: 0.0,
            pitch: -0.45,
            vsync: true,
            cull: true,
            time_gpu: true,
            frames: 0,
            sums: [0.0; 3],
            shown: [0.0; 3],
        })
    }

    fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), -self.yaw.cos() * self.pitch.cos())
    }

    fn update(&mut self, renderer: &mut gfx::Renderer, log: &mut Log, pad: &SceCtrlData, pressed: u32, dt: f32) {
        if pressed & SCE_CTRL_TRIANGLE != 0 {
            self.vsync = !self.vsync;
            renderer.set_vsync(self.vsync);
        }
        if pressed & SCE_CTRL_SQUARE != 0 {
            self.cull = !self.cull;
        }
        if pressed & SCE_CTRL_CIRCLE != 0 {
            self.time_gpu = !self.time_gpu;
        }
        if pressed & SCE_CTRL_CROSS != 0 {
            // The GPU may still read the old buffers: wait before they are freed.
            renderer.finish();
            match Self::load(renderer, log, self.seed + 1) {
                Ok((scene, world, summary)) => {
                    (self.scene, self.world, self.summary) = (scene, world, summary);
                    self.seed += 1;
                }
                Err(error) => log.line(&format!("scene load failed: {error}")),
            }
        }
        self.yaw += axis(pad.rx) * 2.2 * dt;
        self.pitch = (self.pitch - axis(pad.ry) * 1.8 * dt).clamp(-1.5, 1.5);
        let right = Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin());
        let lift = (pad.buttons & SCE_CTRL_RTRIGGER != 0) as i32 - (pad.buttons & SCE_CTRL_LTRIGGER != 0) as i32;
        self.position += (self.forward() * -axis(pad.ly) + right * axis(pad.lx) + Vec3::Y * lift as f32) * 16.0 * dt;
        // Stay above the ground: this is a fly camera, not player physics.
        let side = terrain::SIDE as f32;
        if self.position.x >= 0.0 && self.position.z >= 0.0 && self.position.x < side && self.position.z < side {
            self.position.y = self.position.y.max(self.world.surface_at(self.position.x, self.position.z) + 1.7);
        }
    }

    /// Draws the frame; `started` is when this frame's CPU work began.
    fn draw(&mut self, renderer: &mut gfx::Renderer, log: &mut Log, started: Instant, dt: f32) {
        let wvp = Mat4::perspective_rh_gl(70f32.to_radians(), 960.0 / 544.0, 0.1, 400.0) * Mat4::look_to_rh(self.position, self.forward(), Vec3::Y);
        let drawn = renderer.draw(&self.scene, wvp, self.cull);
        let (user, cdram) = free_memory();
        let [frame_ms, cpu_ms, gpu_ms] = self.shown;
        let fps = if frame_ms > 0.0 { 1000.0 / frame_ms } else { 0.0 };
        let on = |flag: bool| if flag { "oui" } else { "non" };

        renderer.rect(0, 0, 960, 128, 0x8000_0000);
        renderer.text(16, 26, WARN, 1.0, &format!("Horizon Vita {VERSION} - test de rendu 3D, PAS le jeu"));
        renderer.text(
            16,
            48,
            TEXT,
            1.0,
            &format!(
                "{fps:.1} FPS   frame {frame_ms:.1} ms   cpu {cpu_ms:.1} ms   gpu {}",
                if self.time_gpu { format!("{gpu_ms:.1} ms") } else { "non mesuré".into() }
            ),
        );
        renderer.text(
            16,
            70,
            TEXT,
            1.0,
            &format!(
                "chunks {}/{}   triangles {}   vsync {}   culling {}",
                drawn.chunks,
                self.scene.chunk_count(),
                drawn.triangles,
                on(self.vsync),
                on(self.cull)
            ),
        );
        renderer.text(16, 92, TEXT, 1.0, &format!("RAM libre {user} KiB   VRAM libre {cdram} KiB   tas Rust {} KiB", heap::LIVE.load(Relaxed) / 1024));
        renderer.text(16, 114, TEXT, 1.0, &self.summary);
        renderer.rect(0, 490, 960, 54, 0x8000_0000);
        renderer.text(16, 510, TEXT, 1.0, "Sticks : bouger / regarder   L R : descendre / monter   Croix : autre terrain   START : menu");
        renderer.text(16, 532, TEXT, 1.0, "Triangle : vsync   Carré : culling   Rond : mesure GPU");

        let cpu = started.elapsed();
        let gpu = renderer.end(self.time_gpu);
        self.frames += 1;
        self.sums[0] += dt * 1000.0;
        self.sums[1] += cpu.as_secs_f32() * 1000.0;
        self.sums[2] += gpu.map_or(0.0, |g| g.as_secs_f32() * 1000.0);
        if self.frames == WINDOW {
            self.shown = self.sums.map(|sum| sum / WINDOW as f32);
            (self.frames, self.sums) = (0, [0.0; 3]);
            let [frame_ms, cpu_ms, gpu_ms] = self.shown;
            log.sample(&format!(
                "fps {:.1} frame {frame_ms:.2} cpu {cpu_ms:.2} gpu {} chunks {}/{} tris {} vsync {} cull {} free {user}/{cdram} KiB pos {:.0},{:.0},{:.0}",
                1000.0 / frame_ms.max(0.001),
                if self.time_gpu { format!("{gpu_ms:.2}") } else { "off".into() },
                drawn.chunks,
                self.scene.chunk_count(),
                drawn.triangles,
                self.vsync,
                self.cull,
                self.position.x,
                self.position.y,
                self.position.z
            ));
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Menu,
    Game,
    Folders,
    Diagnostic,
}

fn header(renderer: &mut gfx::Renderer, note: &str) {
    renderer.rect(0, 0, 960, 544, BACKGROUND);
    renderer.rect(0, 0, 960, 9, TEAL);
    renderer.rect(0, 9, 960, 95, HEADER);
    renderer.text(48, 76, TEXT, 2.4, "HORIZON VITA");
    renderer.text(760, 66, DIM, 1.0, &format!("DEV {VERSION}"));
    renderer.rect(48, 122, 864, 46, rgb(49, 39, 30));
    renderer.text(66, 152, WARN, 1.0, note);
}

fn footer(renderer: &mut gfx::Renderer, hint: &str) {
    renderer.rect(48, 483, 864, 2, rgb(65, 87, 95));
    renderer.text(48, 514, DIM, 1.0, hint);
}

pub fn run() {
    let _ = std::fs::create_dir_all(DATA);
    let mut log = Log(std::fs::File::create(format!("{DATA}/horizon.log")).ok(), 0);
    let log_ok = log.0.is_some();
    std::panic::set_hook(Box::new(|info| {
        let _ = std::fs::write("ux0:data/HorizonVita/horizon-panic.txt", format!("{info}\n"));
    }));
    log.line(&format!("Horizon Vita {VERSION} - menu, diagnostics, folders and GXM 3D test; no game engine yet"));
    let (user, cdram) = free_memory();
    log.line(&format!("free before graphics: user {user} KiB, cdram {cdram} KiB"));

    let mut renderer = gfx::Renderer::new();
    log.line(&format!("vita2d/GXM initialised, system font {}", if renderer.has_font() { "loaded" } else { "MISSING: bitmap fallback" }));
    unsafe {
        sceCtrlSetSamplingMode(SCE_CTRL_MODE_ANALOG);
        sceTouchSetSamplingState(SCE_TOUCH_PORT_FRONT, SCE_TOUCH_SAMPLING_STATE_START);
        sceTouchSetSamplingState(SCE_TOUCH_PORT_BACK, SCE_TOUCH_SAMPLING_STATE_START);
    }
    let (user, cdram) = free_memory();
    log.line(&format!("free after graphics: user {user} KiB, cdram {cdram} KiB"));

    let mut screen = Screen::Menu;
    let mut selected = 0usize;
    let mut game: Option<Game> = None;
    let mut game_error: Option<String> = None;
    let mut folders = scan_folders(&mut log);
    // (value, microseconds) of the horizon-core self-test, run when Diagnostic first opens.
    let mut noise: Option<(u32, u128)> = None;
    let mut previous = u32::MAX; // ignore buttons already held at launch
    let mut last = Instant::now();

    'frames: loop {
        let started = Instant::now();
        let dt = (started - last).as_secs_f32().min(0.1);
        last = started;

        let mut pad: SceCtrlData = unsafe { core::mem::zeroed() };
        let have_pad = unsafe { sceCtrlPeekBufferPositive(0, &mut pad, 1) } > 0;
        if !have_pad {
            // Neutral sticks, so a failed read cannot move the camera.
            (pad.lx, pad.ly, pad.rx, pad.ry) = (128, 128, 128, 128);
        }
        let pressed = if have_pad { pad.buttons & !previous } else { 0 };
        if have_pad {
            previous = pad.buttons;
        }

        match screen {
            Screen::Menu => {
                if pressed & SCE_CTRL_UP != 0 {
                    selected = (selected + 3) % 4;
                }
                if pressed & SCE_CTRL_DOWN != 0 {
                    selected = (selected + 1) % 4;
                }
                if pressed & SCE_CTRL_CROSS != 0 {
                    log.line(&format!("menu item {selected}"));
                    match selected {
                        0 => {
                            if game.is_none() {
                                match Game::new(&mut renderer, &mut log) {
                                    Ok(new) => game = Some(new),
                                    Err(error) => {
                                        log.line(&format!("3D test failed to start: {error}"));
                                        game_error = Some(error);
                                    }
                                }
                            }
                            if let Some(game) = &game {
                                renderer.set_vsync(game.vsync);
                                screen = Screen::Game;
                            }
                        }
                        1 => {
                            folders = scan_folders(&mut log);
                            screen = Screen::Folders;
                        }
                        2 => {
                            if noise.is_none() {
                                let start = Instant::now();
                                let value = terrain::noise_checksum(FIRST_SEED);
                                noise = Some((value, start.elapsed().as_micros()));
                                log.line(&format!("horizon-core noise self-test: {value:08X} (PC {EXPECTED_NOISE:08X}) in {} us", start.elapsed().as_micros()));
                            }
                            screen = Screen::Diagnostic;
                        }
                        _ => break 'frames,
                    }
                }
            }
            Screen::Game => {
                if pressed & SCE_CTRL_START != 0 {
                    renderer.set_vsync(true);
                    screen = Screen::Menu;
                } else if let Some(game) = &mut game {
                    game.update(&mut renderer, &mut log, &pad, pressed, dt);
                }
            }
            Screen::Folders => {
                if pressed & SCE_CTRL_CROSS != 0 {
                    folders = scan_folders(&mut log);
                }
                if pressed & SCE_CTRL_CIRCLE != 0 {
                    screen = Screen::Menu;
                }
            }
            Screen::Diagnostic => {
                if pressed & SCE_CTRL_CIRCLE != 0 {
                    screen = Screen::Menu;
                }
            }
        }

        renderer.begin();
        match screen {
            Screen::Game => {
                if let Some(game) = &mut game {
                    game.draw(&mut renderer, &mut log, started, dt);
                    continue 'frames; // `draw` ended the frame itself, to time it
                }
            }
            Screen::Menu => {
                header(&mut renderer, "Le moteur de jeu n'est pas encore porté. JOUER ouvre le test de rendu 3D.");
                let items = [
                    ("JOUER", "test de rendu 3D"),
                    ("MODS", "dossiers sur la carte"),
                    ("DIAGNOSTIC", "commandes, mémoire, calcul"),
                    ("QUITTER", ""),
                ];
                for (index, (label, note)) in items.iter().enumerate() {
                    let y = 196 + index as i32 * 65;
                    let on = index == selected;
                    renderer.rect(48, y, 864, 53, if on { SELECTED } else { PANEL });
                    renderer.rect(48, y, 6, 53, if on { TEAL } else { rgb(65, 78, 92) });
                    renderer.text(72, y + 37, TEXT, 1.5, label);
                    renderer.text(600, y + 34, if index == 0 { WARN } else { DIM }, 1.0, note);
                }
                match &game_error {
                    Some(error) => renderer.text(48, 474, BAD, 1.0, &format!("Le test 3D n'a pas démarré : {error}")),
                    None => {}
                }
                footer(&mut renderer, "Haut / Bas : choisir     Croix : ouvrir");
            }
            Screen::Folders => {
                header(&mut renderer, "Rien n'est encore chargé depuis ces dossiers : le moteur n'est pas porté.");
                renderer.text(48, 200, TEXT, 1.0, "Depuis le PC (VitaShell, USB ou FTP), dépose tes fichiers ici :");
                for (index, ((label, folder), count)) in FOLDERS.iter().zip(folders).enumerate() {
                    let y = 214 + index as i32 * 58;
                    renderer.rect(48, y, 864, 50, PANEL);
                    renderer.text(64, y + 21, TEAL, 1.0, label);
                    renderer.text(64, y + 43, TEXT, 1.0, &format!("{DATA}/{folder}"));
                    match count {
                        Some(count) => renderer.text(760, y + 32, DIM, 1.0, &format!("{count} élément(s)")),
                        None => renderer.text(760, y + 32, BAD, 1.0, "illisible"),
                    }
                }
                renderer.text(48, 470, WARN, 1.0, "Les .jar Fabric / Forge / NeoForge ne fonctionnent dans Horizon sur aucune machine.");
                footer(&mut renderer, "Croix : relire     Rond : retour");
            }
            Screen::Diagnostic => {
                header(&mut renderer, "Diagnostic : commandes, mémoire et calcul Horizon sur cette console.");
                let mut y = 198;
                let mut row = |renderer: &mut gfx::Renderer, colour: u32, text: String| {
                    renderer.text(48, y, colour, 1.0, &text);
                    y += 25;
                };
                if have_pad {
                    row(&mut renderer, TEXT, format!("Boutons {:08X}     stick gauche {:3} {:3}     stick droit {:3} {:3}", pad.buttons, pad.lx, pad.ly, pad.rx, pad.ry));
                } else {
                    row(&mut renderer, BAD, "Lecture des commandes impossible".into());
                }
                let mut touch: SceTouchData = unsafe { core::mem::zeroed() };
                let front = (unsafe { sceTouchPeek(SCE_TOUCH_PORT_FRONT, &mut touch, 1) } >= 0).then(|| (touch.reportNum, touch.report));
                let back = (unsafe { sceTouchPeek(SCE_TOUCH_PORT_BACK, &mut touch, 1) } >= 0).then_some(touch.reportNum);
                match (&front, back) {
                    (Some((front, _)), Some(back)) => row(&mut renderer, TEXT, format!("Tactile avant {front}     tactile arrière {back}")),
                    _ => row(&mut renderer, BAD, "Lecture du tactile impossible".into()),
                }
                let (user, cdram) = free_memory();
                row(&mut renderer, TEXT, format!("RAM libre {user} KiB     mémoire vidéo libre {cdram} KiB"));
                row(&mut renderer, TEXT, format!("Tas Rust : {} KiB utilisés, pic {} KiB", heap::LIVE.load(Relaxed) / 1024, heap::PEAK.load(Relaxed) / 1024));
                if let Some((value, micros)) = noise {
                    let same = value == EXPECTED_NOISE;
                    row(
                        &mut renderer,
                        if same { GOOD } else { BAD },
                        format!(
                            "Bruit Horizon (horizon-core) : {value:08X}, PC {EXPECTED_NOISE:08X} : {}     {micros} µs",
                            if same { "IDENTIQUE" } else { "DIFFÉRENT" }
                        ),
                    );
                }
                row(&mut renderer, if log_ok { TEXT } else { BAD }, format!("Journal {DATA}/horizon.log : {}", if log_ok { "ok" } else { "ÉCHEC" }));
                let font = renderer.has_font();
                row(&mut renderer, if font { TEXT } else { WARN }, format!("Police système : {}", if font { "chargée" } else { "absente, police de secours" }));
                // Front touch reports are 1920x1088; the screen is half that.
                if let Some((count, reports)) = front {
                    for report in reports.iter().take(count.min(8) as usize) {
                        renderer.rect(report.x as i32 / 2 - 7, report.y as i32 / 2 - 7, 14, 14, TEAL);
                    }
                }
                footer(&mut renderer, "Rond : retour");
            }
        }
        renderer.end(false);
    }
    renderer.finish();
    log.line("clean exit");
}
