//! A resource pack's `core/text` shader on the GUI's text (`RenderPipelines.GUI_TEXT`, the shader
//! with `IS_GUI`): servers' HUDs that their pack's vertex shader moves to the screen's corners,
//! and text effects in its fragment shader. Without one the text draws natively.

use std::path::Path;
use std::sync::Arc;

use horizon_shaders::{CoreProgram, Glue};

use super::{HudBatch, HudVertex};

mod desktop;
pub(super) use desktop::PackText;

/// GUI text's vertex format (`POSITION_TEX_COLOR`) from the HUD's vertices. A glyph is two
/// triangles here, its corners top left, top right, bottom right, top left, bottom right, bottom
/// left; vanilla's quads go top left, bottom left, bottom right, top right, which is what
/// `gl_VertexID % 4` tells a pack's shader. The draw passes its first vertex as its instance.
const GLUE: Glue = Glue {
    inputs: &[("Position", "vec3"), ("Color", "vec4"), ("UV0", "vec2")],
    prelude: "layout(location = 0) in vec2 hz_pos;\n\
              layout(location = 1) in vec2 hz_uv;\n\
              layout(location = 2) in vec4 hz_color;\n\
              const int hz_corner[6] = int[6](0, 3, 2, 0, 2, 1);\n\
              int hz_vertex_id() {\n\
                  int i = int(gl_VertexIndex) - int(gl_InstanceIndex);\n\
                  return i / 6 * 4 + hz_corner[i % 6];\n\
              }",
    setup: "Position = vec3(hz_pos, 0.0); Color = hz_color; UV0 = hz_uv;",
    // GL's clip depth runs from -w to w, wgpu's from 0 to w; GUI text has no depth test.
    finish: "gl_Position.z = 0.5 * gl_Position.w;",
    vertex_id: "hz_vertex_id()",
};

/// The packs' text shader, when a pack replaces either stage.
pub(super) fn load(root: &Path, defaults: &Path) -> Option<Arc<CoreProgram>> {
    load_core(root, defaults, "text", &["IS_GUI"], &GLUE)
}

/// The packs' core shader `name` (`shaders/core/<name>.vsh` and `.fsh`) over `defaults` (the
/// game's own resources, which have vanilla's and its includes), when a pack replaces either
/// stage.
pub(super) fn load_core(
    root: &Path,
    defaults: &Path,
    name: &str,
    defines: &[&str],
    glue: &Glue,
) -> Option<Arc<CoreProgram>> {
    if root == defaults {
        return None;
    }
    let read = |base: &Path, path: &str| std::fs::read_to_string(base.join(path)).ok();
    let own =
        |path: &str| read(root, path).filter(|text| read(defaults, path).as_ref() != Some(text));
    let vertex_path = format!("assets/minecraft/shaders/core/{name}.vsh");
    let fragment_path = format!("assets/minecraft/shaders/core/{name}.fsh");
    let (own_vertex, own_fragment) = (own(&vertex_path), own(&fragment_path));
    if own_vertex.is_none() && own_fragment.is_none() {
        return None;
    }
    let vertex = own_vertex.or_else(|| read(defaults, &vertex_path))?;
    let fragment = own_fragment.or_else(|| read(defaults, &fragment_path))?;
    let include = |id: &str| {
        let (namespace, path) = id.split_once(':')?;
        let path = format!("assets/{namespace}/{path}");
        read(root, &path).or_else(|| read(defaults, &path))
    };
    match horizon_shaders::translate_core(&vertex, &fragment, defines, &include, glue) {
        Ok(program) => {
            log::info!("the resource pack's {name} shader is used");
            Some(Arc::new(program))
        }
        Err(e) => {
            log::warn!("the resource pack's {name} shader is not used: {e}");
            None
        }
    }
}

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
