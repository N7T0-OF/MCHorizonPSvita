//! A 960x544 framebuffer with an 8x8 bitmap font.
//!
//! The framebuffer setup and `font.bin` follow the vita-rust examples
//! (https://github.com/vita-rust/examples, MIT OR Apache-2.0).

use core::ffi::c_void;
use vitasdk_sys::*;

pub const WIDTH: usize = 960;
pub const HEIGHT: usize = 544;
const FB_BYTES: usize = 2 * 1024 * 1024;
static FONT: &[u8; 2048] = include_bytes!("font.bin");

pub struct Screen {
    pixels: &'static mut [u32],
    block: SceUID,
}

impl Screen {
    pub fn new() -> Self {
        let mut base: *mut c_void = core::ptr::null_mut();
        let block = unsafe {
            sceKernelAllocMemBlock(
                c"horizon-probe-fb".as_ptr() as _,
                SCE_KERNEL_MEMBLOCK_TYPE_USER_CDRAM_RW as _,
                FB_BYTES as _,
                core::ptr::null_mut(),
            )
        };
        assert!(block >= 0, "framebuffer allocation failed: {block:#x}");
        unsafe { sceKernelGetMemBlockBase(block, &mut base) };
        assert!(!base.is_null(), "framebuffer has no base address");
        let frame = SceDisplayFrameBuf {
            size: core::mem::size_of::<SceDisplayFrameBuf>() as _,
            base,
            pitch: WIDTH as _,
            pixelformat: 0,
            width: WIDTH as _,
            height: HEIGHT as _,
        };
        unsafe { sceDisplaySetFrameBuf(&frame, SCE_DISPLAY_SETBUF_NEXTFRAME as _) };
        // SAFETY: the block is 2 MiB, page aligned, and lives as long as `Screen`.
        let pixels = unsafe { core::slice::from_raw_parts_mut(base as *mut u32, WIDTH * HEIGHT) };
        Screen { pixels, block }
    }

    pub fn clear(&mut self, colour: u32) {
        self.pixels.fill(colour);
    }

    pub fn block(&mut self, x: usize, y: usize, size: usize, colour: u32) {
        for row in y..(y + size).min(HEIGHT) {
            for col in x..(x + size).min(WIDTH) {
                self.pixels[row * WIDTH + col] = colour;
            }
        }
    }

    pub fn text(&mut self, mut x: usize, y: usize, scale: usize, colour: u32, text: &str) {
        for byte in text.bytes() {
            let glyph = &FONT[(byte as usize & 0xFF) * 8..][..8];
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..8 {
                    if bits & (0x80 >> col) != 0 {
                        self.block(x + col * scale, y + row * scale, scale, colour);
                    }
                }
            }
            x += 8 * scale;
        }
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        unsafe { sceKernelFreeMemBlock(self.block) };
    }
}
