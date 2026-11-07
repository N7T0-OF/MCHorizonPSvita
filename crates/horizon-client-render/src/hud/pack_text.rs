//! A resource pack's `core/text` shader on the GUI's text (`RenderPipelines.GUI_TEXT`, the shader
//! with `IS_GUI`): servers' HUDs that their pack's vertex shader moves to the screen's corners,
//! and text effects in its fragment shader. Without one the text draws natively.

use super::{HudBatch, HudVertex};

mod desktop;
pub(super) use desktop::PackText;

/// The std140 blocks: `Globals` (`GlobalSettingsUniform.update`: the camera at the origin,
/// `GlintAlpha`, `GameTime`, `ScreenSize`), `Projection` (`GuiRenderer`'s orthographic GUI
/// projection, 1000 to 11000 deep, in GL's depth range), `DynamicTransforms` (the GUI's model view
/// moving 11000 back, colour modulator white) and an empty `Fog`.
/// `scale`: target pixels per GUI pixel (`batch.scale`, times `HudRenderer::zoom`).
fn uniforms(batch: &HudBatch, size: [u32; 2], scale: f32) -> Vec<u8> {
    let mut data = vec![0f32; 256];
    let globals = 0;
    data[globals + 3] = batch.glint_alpha;
    data[globals + 7] = batch.game_time;
    data[globals + 8] = size[0] as f32;
    data[globals + 9] = size[1] as f32;
    // Matrix4f.setOrtho(0, width, height, 0, near, far) with the GUI's scale, columns.
    let (w, h) = (size[0] as f32 / scale, size[1] as f32 / scale);
    let (near, far) = (1000.0f32, 11000.0f32);
    let projection = 64;
    data[projection] = 2.0 / w;
    data[projection + 5] = -2.0 / h;
    data[projection + 10] = 2.0 / (near - far);
    data[projection + 12] = -1.0;
    data[projection + 13] = 1.0;
    data[projection + 14] = (far + near) / (near - far);
    data[projection + 15] = 1.0;
    let transforms = 128;
    for (matrix, z) in [(transforms, -11000.0), (transforms + 16, 0.0)] {
        data[matrix] = 1.0;
        data[matrix + 5] = 1.0;
        data[matrix + 10] = 1.0;
        data[matrix + 14] = z;
        data[matrix + 15] = 1.0;
    }
    data[transforms + 32..transforms + 36].fill(1.0);
    bytemuck::cast_slice(&data).to_vec()
}

/// `range` split into runs of GUI text and of the rest, in order.
pub(super) fn text_runs(
    vertices: &[HudVertex],
    range: std::ops::Range<u32>,
) -> Vec<(std::ops::Range<u32>, bool)> {
    let mut runs: Vec<(std::ops::Range<u32>, bool)> = Vec::new();
    for i in range {
        let text = vertices.get(i as usize).is_some_and(|v| v.text != 0);
        match runs.last_mut() {
            Some((run, t)) if *t == text => run.end = i + 1,
            _ => runs.push((i..i + 1, text)),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gui_projection_maps_gui_pixels_to_the_screen() {
        let mut batch = HudBatch::new([1920, 1080]);
        batch.scale = 4.0;
        let data: Vec<f32> =
            bytemuck::cast_slice(&uniforms(&batch, [1920, 1080], batch.scale)).to_vec();
        let column = |matrix: usize, c: usize| &data[matrix + 4 * c..matrix + 4 * c + 4];
        let apply = |matrix: usize, v: [f32; 4]| -> [f32; 4] {
            std::array::from_fn(|r| (0..4).map(|c| column(matrix, c)[r] * v[c]).sum())
        };
        // ProjMat * ModelViewMat * (x, y, 0, 1) for the GUI's corners: 480 x 270 GUI pixels.
        for ([x, y], [cx, cy]) in [([0.0, 0.0], [-1.0, 1.0]), ([480.0, 270.0], [1.0, -1.0])] {
            let clip = apply(64, apply(128, [x, y, 0.0, 1.0]));
            assert!(
                (clip[0] - cx).abs() < 1e-5 && (clip[1] - cy).abs() < 1e-5,
                "{clip:?}"
            );
            // At GL's far plane, as vanilla's GUI is.
            assert!((clip[2] - 1.0).abs() < 1e-4, "{clip:?}");
        }
        // A pack finds the GUI scale as ScreenSize.x * ProjMat[0][0] / 2.
        assert!((data[8] * data[64] / 2.0 - 4.0).abs() < 1e-4);
    }

    #[test]
    fn text_splits_from_the_rest() {
        let vertex = |text| HudVertex {
            pos: [0.0; 2],
            uv: [0.0; 2],
            color: 0,
            depth: 0.0,
            glint_uv: [0.0; 2],
            text,
        };
        let vertices: Vec<HudVertex> = [0, 0, 1, 1, 1, 0].into_iter().map(vertex).collect();
        assert_eq!(
            text_runs(&vertices, 1..6),
            [(1..2, false), (2..5, true), (5..6, false)]
        );
    }
}
