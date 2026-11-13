//! Draws a packed world (`.hvw`) to a PNG with a small software rasteriser, using the
//! same matrices, quad triangulation and draw order as the console. It checks a pack,
//! and the reading of its format, without a round trip to the PS Vita.
//!
//! usage: preview <world.hvw> <out.png> [yaw-degrees] [pitch-degrees] [lift]

use glam::{Mat4, Vec3, Vec4};

const W: usize = 960;
const H: usize = 544;

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: preview <world.hvw> <out.png> [yaw-degrees] [pitch-degrees] [lift]";
    let pack = std::fs::read(args.next().expect(usage)).expect("pack file");
    let out = args.next().expect(usage);
    let yaw = args.next().map_or(0.0f32, |s| s.parse().unwrap()).to_radians();
    let pitch = args.next().map_or(-20.0f32, |s| s.parse().unwrap()).to_radians();
    let lift: f32 = args.next().map_or(0.0, |s| s.parse().unwrap());

    assert_eq!(&pack[0..4], b"HVW1", "not a Horizon Vita world pack");
    assert_eq!(u32_at(&pack, 4), 1, "pack version");
    let (aw, ah) = (u32_at(&pack, 8) as usize, u32_at(&pack, 12) as usize);
    let (batch_count, quads) = (u32_at(&pack, 16) as usize, u32_at(&pack, 20) as usize);
    let camera = Vec3::new(f32_at(&pack, 24), f32_at(&pack, 28) + lift, f32_at(&pack, 32));
    let atlas = &pack[64..64 + aw * ah * 4];
    let table = 64 + aw * ah * 4;
    let vertices = table + batch_count * 36;
    assert_eq!(pack.len(), vertices + quads * 4 * 20, "pack length");

    let forward = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
    let wvp = Mat4::perspective_rh_gl(70f32.to_radians(), W as f32 / H as f32, 0.1, 400.0) * Mat4::look_to_rh(camera, forward, Vec3::Y);

    let mut colour = vec![[120u8, 180, 235]; W * H];
    let mut depth = vec![f32::MAX; W * H];
    let mut drawn = [0usize; 3];
    for batch in 0..batch_count {
        let entry = table + batch * 36;
        let (mode, first, count) = (u32_at(&pack, entry + 24), u32_at(&pack, entry + 28) as usize, u32_at(&pack, entry + 32) as usize);
        for quad in first..first + count {
            let corner: [(Vec4, [f32; 2]); 4] = std::array::from_fn(|i| {
                let at = vertices + (quad * 4 + i) * 20;
                let position = Vec3::new(f32_at(&pack, at), f32_at(&pack, at + 4), f32_at(&pack, at + 8));
                (wvp * position.extend(1.0), [f32_at(&pack, at + 12), f32_at(&pack, at + 16)])
            });
            for triangle in [[0, 1, 2], [0, 2, 3]] {
                let [a, b, c] = triangle.map(|i| corner[i]);
                if a.0.w <= 0.1 || b.0.w <= 0.1 || c.0.w <= 0.1 {
                    continue; // no near clipping in this preview
                }
                let screen = [a, b, c].map(|(p, _)| [(p.x / p.w * 0.5 + 0.5) * W as f32, (0.5 - p.y / p.w * 0.5) * H as f32, p.z / p.w]);
                let area = (screen[1][0] - screen[0][0]) * (screen[2][1] - screen[0][1]) - (screen[2][0] - screen[0][0]) * (screen[1][1] - screen[0][1]);
                if area.abs() < 1e-6 {
                    continue;
                }
                let min_x = screen.iter().map(|s| s[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
                let max_x = (screen.iter().map(|s| s[0]).fold(f32::MIN, f32::max).ceil() as isize).min(W as isize - 1);
                let min_y = screen.iter().map(|s| s[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
                let max_y = (screen.iter().map(|s| s[1]).fold(f32::MIN, f32::max).ceil() as isize).min(H as isize - 1);
                let inverse_w = [1.0 / a.0.w, 1.0 / b.0.w, 1.0 / c.0.w];
                let uv = [a.1, b.1, c.1];
                for y in min_y as isize..=max_y {
                    for x in min_x as isize..=max_x {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let edge = |i: usize, j: usize| (screen[j][0] - screen[i][0]) * (py - screen[i][1]) - (px - screen[i][0]) * (screen[j][1] - screen[i][1]);
                        let weights = [edge(1, 2) / area, edge(2, 0) / area, edge(0, 1) / area];
                        if weights.iter().any(|&w| w < 0.0) {
                            continue;
                        }
                        let z = weights[0] * screen[0][2] + weights[1] * screen[1][2] + weights[2] * screen[2][2];
                        let pixel = y as usize * W + x as usize;
                        if z > depth[pixel] {
                            continue;
                        }
                        let w = weights[0] * inverse_w[0] + weights[1] * inverse_w[1] + weights[2] * inverse_w[2];
                        let u = (weights[0] * uv[0][0] * inverse_w[0] + weights[1] * uv[1][0] * inverse_w[1] + weights[2] * uv[2][0] * inverse_w[2]) / w;
                        let v = (weights[0] * uv[0][1] * inverse_w[0] + weights[1] * uv[1][1] * inverse_w[1] + weights[2] * uv[2][1] * inverse_w[2]) / w;
                        let texel = ((v * ah as f32) as usize).min(ah - 1) * aw + ((u * aw as f32) as usize).min(aw - 1);
                        let [r, g, b, alpha] = atlas[texel * 4..texel * 4 + 4] else { unreachable!() };
                        // The console blends by alpha and writes depth for every pixel of a quad.
                        depth[pixel] = z;
                        let under = colour[pixel];
                        let mix = |over: u8, under: u8| ((over as u32 * alpha as u32 + under as u32 * (255 - alpha as u32)) / 255) as u8;
                        colour[pixel] = [mix(r, under[0]), mix(g, under[1]), mix(b, under[2])];
                    }
                }
                drawn[mode as usize] += 1;
            }
        }
    }
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&out).expect("output file")), W as u32, H as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&colour.concat()).unwrap();
    println!("{out}: camera {camera}, triangles drawn opaque {} cutout {} translucent {}", drawn[0], drawn[1], drawn[2]);
}
