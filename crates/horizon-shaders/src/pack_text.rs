//! CPU resource-pack core shader loading for Horizon GUI presentation.

use std::path::Path;
use std::sync::Arc;

use crate::{CoreProgram, Glue};

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
pub fn load(root: &Path, defaults: &Path) -> Option<Arc<CoreProgram>> {
    load_core(root, defaults, "text", &["IS_GUI"], &GLUE)
}

/// The packs' core shader `name` (`shaders/core/<name>.vsh` and `.fsh`) over `defaults` (the
/// game's own resources, which have vanilla's and its includes), when a pack replaces either
/// stage.
pub fn load_core(
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
    match crate::translate_core(&vertex, &fragment, defines, &include, glue) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_shader_is_skipped_and_pack_override_loads() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("horizon-pack-text-{}-{unique}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let root = Scratch(root);
        let defaults = root.0.join("defaults");
        let pack = root.0.join("pack");
        let path = "assets/minecraft/shaders/core";
        std::fs::create_dir_all(defaults.join(path)).unwrap();
        std::fs::create_dir_all(pack.join(path)).unwrap();
        let vertex =
            "#version 330\nin vec3 Position;\nvoid main() { gl_Position = vec4(Position, 1.0); }\n";
        let fragment =
            "#version 330\nout vec4 fragColor;\nvoid main() { fragColor = vec4(1.0); }\n";
        for base in [&defaults, &pack] {
            std::fs::write(base.join(path).join("text.vsh"), vertex).unwrap();
            std::fs::write(base.join(path).join("text.fsh"), fragment).unwrap();
        }
        assert!(load(&defaults, &defaults).is_none());
        assert!(load(&pack, &defaults).is_none());
        std::fs::write(
            pack.join(path).join("text.fsh"),
            "#version 330\nout vec4 fragColor;\nvoid main() { fragColor = vec4(0.5); }\n",
        )
        .unwrap();
        assert!(load(&pack, &defaults).is_some());
    }
}
