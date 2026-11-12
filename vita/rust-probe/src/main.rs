//! Horizon Vita Rust probe. It is NOT the game: it proves that Rust `std` and
//! Horizon's `horizon-core` run on a PS Vita, and measures what they cost there.
//!
//! On the PC (`cargo run`) it prints the checksum the console must show.

mod probe;

#[cfg(target_os = "vita")]
mod screen;

/// Counts what the Rust heap holds, so the probe can report it.
mod heap {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    pub static LIVE: AtomicUsize = AtomicUsize::new(0);
    pub static PEAK: AtomicUsize = AtomicUsize::new(0);
    pub static CALLS: AtomicUsize = AtomicUsize::new(0);

    pub struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                let live = LIVE.fetch_add(layout.size(), Relaxed) + layout.size();
                PEAK.fetch_max(live, Relaxed);
                CALLS.fetch_add(1, Relaxed);
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

#[cfg(not(target_os = "vita"))]
fn main() {
    let start = std::time::Instant::now();
    let map = probe::noise_map(20261112);
    println!(
        "seed 20261112: checksum {:08X}, {} samples, {:?}, heap peak {} bytes",
        map.checksum,
        map.samples.len(),
        start.elapsed(),
        heap::PEAK.load(std::sync::atomic::Ordering::Relaxed)
    );
}

#[cfg(target_os = "vita")]
fn main() {
    vita::run();
}

#[cfg(target_os = "vita")]
mod vita {
    use super::{heap, probe, screen::Screen};
    use std::io::Write as _;
    use std::sync::atomic::Ordering::Relaxed;
    use std::time::Instant;
    use vitasdk_sys::*;

    const LOG: &str = "ux0:data/HorizonVita/rust-probe.log";
    const WHITE: u32 = 0xFFFF_FFFF;
    const DIM: u32 = 0xFFBE_B6AA;
    const WARN: u32 = 0xFF5B_B6FF;

    fn free_memory() -> Option<(i32, i32, i32)> {
        let mut info: SceKernelFreeMemorySizeInfo = unsafe { core::mem::zeroed() };
        info.size = core::mem::size_of::<SceKernelFreeMemorySizeInfo>() as _;
        (unsafe { sceKernelGetFreeMemorySize(&mut info) } >= 0)
            .then(|| (info.size_user as i32 / 1024, info.size_cdram as i32 / 1024, info.size_phycont as i32 / 1024))
    }

    fn log(file: &mut Option<std::fs::File>, line: &str) {
        if let Some(f) = file {
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
        }
    }

    pub fn run() {
        let _ = std::fs::create_dir_all("ux0:data/HorizonVita");
        let mut file = std::fs::File::create(LOG).ok();
        let log_ok = file.is_some();
        log(&mut file, "Horizon Vita Rust probe 0.0.1 - horizon-core on console, not the game");
        std::panic::set_hook(Box::new(|info| {
            let _ = std::fs::write("ux0:data/HorizonVita/rust-probe-panic.txt", format!("{info}\n"));
        }));

        let mut screen = Screen::new();
        unsafe { sceCtrlSetSamplingMode(SCE_CTRL_MODE_ANALOG as _) };
        let mut seed: i64 = 20261112;
        let mut previous = u32::MAX;
        let mut redraw = true;
        loop {
            if redraw {
                redraw = false;
                let start = Instant::now();
                let map = probe::noise_map(seed);
                let elapsed = start.elapsed();
                screen.clear(0xFF19_120D);
                screen.text(24, 16, 3, WHITE, "HORIZON VITA - RUST PROBE");
                screen.text(24, 48, 2, WARN, "NOT THE GAME: horizon-core noise running on this console");
                for z in 0..probe::MAP_H {
                    for x in 0..probe::MAP_W {
                        screen.block(24 + x * 2, 84 + z * 2, 2, probe::colour(map.samples[z * probe::MAP_W + x]));
                    }
                }
                let mut lines: Vec<(u32, String)> = vec![
                    (WHITE, format!("seed {seed}")),
                    (WHITE, format!("checksum {:08X}", map.checksum)),
                    (DIM, "(compare with the PC run)".into()),
                    (WHITE, format!("noise {} us", elapsed.as_micros())),
                    (DIM, format!("{} samples", map.samples.len())),
                    (DIM, String::new()),
                    (WHITE, format!("heap live {} B", heap::LIVE.load(Relaxed))),
                    (WHITE, format!("heap peak {} B", heap::PEAK.load(Relaxed))),
                    (DIM, format!("allocations {}", heap::CALLS.load(Relaxed))),
                    (DIM, String::new()),
                ];
                match free_memory() {
                    Some((user, cdram, phycont)) => {
                        lines.push((WHITE, format!("free user {user} KiB")));
                        lines.push((WHITE, format!("free cdram {cdram} KiB")));
                        lines.push((WHITE, format!("free phycont {phycont} KiB")));
                    }
                    None => lines.push((WARN, "memory query failed".into())),
                }
                lines.push(if log_ok { (DIM, "log file ok".into()) } else { (WARN, "log file FAILED".into()) });
                for (row, (colour, text)) in lines.iter().enumerate() {
                    screen.text(528, 84 + row * 22, 2, *colour, text);
                }
                screen.text(24, 510, 2, DIM, "CROSS: next seed    START: exit");
                log(
                    &mut file,
                    &format!(
                        "seed {seed} checksum {:08X} noise {} us heap_peak {} free {:?}",
                        map.checksum,
                        elapsed.as_micros(),
                        heap::PEAK.load(Relaxed),
                        free_memory()
                    ),
                );
            }
            let mut pad: SceCtrlData = unsafe { core::mem::zeroed() };
            if unsafe { sceCtrlPeekBufferPositive(0, &mut pad, 1) } > 0 {
                let pressed = pad.buttons & !previous;
                previous = pad.buttons;
                if pressed & SCE_CTRL_START as u32 != 0 {
                    break;
                }
                if pressed & SCE_CTRL_CROSS as u32 != 0 {
                    seed += 1;
                    redraw = true;
                }
            }
            unsafe { sceDisplayWaitVblankStart() };
        }
        log(&mut file, "clean exit");
    }
}
