use png::{ColorType, Decoder, Transformations};
use std::{
    error::Error,
    fs::{self, File},
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

const TILE_SIZE: usize = 16;
const ATLAS_WIDTH: usize = TILE_SIZE * 4;
const ATLAS_HEIGHT: usize = TILE_SIZE * 2;
const PLAINS_GRASS: [u8; 3] = [0x7f, 0xb2, 0x38];

fn read_texture(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut decoder = Decoder::new(BufReader::new(File::open(path)?));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let output_size = reader
        .output_buffer_size()
        .ok_or("PNG output size overflows address space")?;
    let mut source = vec![0; output_size];
    let frame = reader.next_frame(&mut source)?;
    if frame.width as usize != TILE_SIZE || frame.height as usize != TILE_SIZE {
        return Err(format!(
            "{} must be 16x16, got {}x{}",
            path.display(),
            frame.width,
            frame.height
        )
        .into());
    }

    let channels = frame.color_type.samples();
    let mut rgba = Vec::with_capacity(TILE_SIZE * TILE_SIZE * 4);
    for pixel in source[..frame.buffer_size()].chunks_exact(channels) {
        match frame.color_type {
            ColorType::Rgba => rgba.extend_from_slice(pixel),
            ColorType::Rgb => rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]),
            ColorType::GrayscaleAlpha => {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]])
            }
            ColorType::Grayscale => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], 255]),
            ColorType::Indexed => return Err("PNG palette was not expanded".into()),
        }
    }
    Ok(rgba)
}

fn tint_grayscale(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[0] == pixel[1] && pixel[1] == pixel[2] {
            for channel in 0..3 {
                pixel[channel] =
                    (u16::from(pixel[channel]) * u16::from(PLAINS_GRASS[channel]) / 255) as u8;
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: atlas_builder <prepared-26.3-root> <output-wiiu-terrain-atlas.rgba>")?;
    let output = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: atlas_builder <prepared-26.3-root> <output-wiiu-terrain-atlas.rgba>")?;
    if args.next().is_some() {
        return Err("too many arguments".into());
    }

    let textures = root.join("assets/minecraft/textures/block");
    let mut grass_top = read_texture(&textures.join("grass_block_top.png"))?;
    tint_grayscale(&mut grass_top);
    let grass_side = read_texture(&textures.join("grass_block_side.png"))?;

    let tiles = [
        grass_top,
        grass_side,
        read_texture(&textures.join("stone.png"))?,
        read_texture(&textures.join("dirt.png"))?,
        read_texture(&textures.join("sand.png"))?,
        read_texture(&textures.join("gravel.png"))?,
        read_texture(&textures.join("oak_log.png"))?,
        read_texture(&textures.join("oak_planks.png"))?,
    ];
    let mut atlas = vec![0; ATLAS_WIDTH * ATLAS_HEIGHT * 4];
    for (index, tile) in tiles.iter().enumerate() {
        let tile_x = (index % 4) * TILE_SIZE;
        let tile_y = (index / 4) * TILE_SIZE;
        for y in 0..TILE_SIZE {
            let source = &tile[y * TILE_SIZE * 4..(y + 1) * TILE_SIZE * 4];
            let destination_start = ((tile_y + y) * ATLAS_WIDTH + tile_x) * 4;
            atlas[destination_start..destination_start + TILE_SIZE * 4].copy_from_slice(source);
        }
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(&output)?;
    file.write_all(&atlas)?;
    println!(
        "Wrote {} ({}x{} RGBA from Horizon's prepared 26.3 textures)",
        output.display(),
        ATLAS_WIDTH,
        ATLAS_HEIGHT
    );
    Ok(())
}
