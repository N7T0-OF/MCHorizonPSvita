//! Backend contracts between Horizon's CPU-side frame/section data and a
//! window-facing renderer.
//!
//! The desktop renderer is one implementation. Platform renderers can consume
//! the same [`Frame`] and [`SectionMesh`] without changing world streaming or
//! frame construction.

use crate::{
    Result,
    chunk::ColumnLayout,
    gfx::{Frame, SectionKey},
    graphics::PresentMode,
    hud::{Rect, cit_glint::GlintTextures},
    mesher::SectionMesh,
    sprites::SpriteAnimation,
};

mod desktop;

/// Surface operations used by Horizon's frame loop, independent of the GPU API.
pub trait PresentationBackend {
    /// Active surface size in physical pixels.
    fn viewport_size(&self) -> [u32; 2];
    /// Presents one Horizon-built CPU frame. `Ok(false)` means the surface did
    /// not accept it this frame (for example, because it is occluded).
    fn present_frame(&mut self, frame: &Frame<'_>) -> Result<bool>;
    fn resize(&mut self, width: u32, height: u32);
    fn release_surface(&mut self);
    fn recreate_surface(&mut self) -> Result<()>;
    fn set_present_mode(&mut self, vsync: bool, mode: PresentMode);
    fn set_gamma(&mut self, gamma: f32);
}

/// CPU-supplied HUD resources and the texture limit needed when preparing them.
pub trait HudBackend {
    fn set_hud_sprite_animations(&mut self, animations: &[(Rect, SpriteAnimation)]);
    fn set_hud_text_shader(&mut self, shader: Option<&horizon_shaders::CoreProgram>);
    fn set_hud_cit_glints(&mut self, textures: Option<&GlintTextures>);
    fn set_lines_shader(&mut self, shader: Option<&horizon_shaders::CoreProgram>);
    fn max_texture_dimension_2d(&self) -> u32;
}

/// Dynamic chunk-mesh lifecycle. Upload replaces the mesh for a section; a
/// backend reports allocation/conversion errors so Horizon can retain the CPU
/// mesh and retry it on a later frame.
pub trait TerrainBackend {
    /// Uploads a complete section replacement. On error, the caller keeps the
    /// CPU mesh and may retry; the backend must not retain the borrowed mesh or
    /// leave a partially installed section, and retrying the same key is safe.
    fn upload_section(&mut self, key: SectionKey, mesh: &SectionMesh) -> Result<()>;
    fn clear_sections(&mut self, layout: ColumnLayout);
    fn remove_columns(&mut self, columns: &mut dyn Iterator<Item = (i32, i32)>);
}

/// Submit Horizon's frame through its selected presentation backend.
pub fn present(backend: &mut dyn PresentationBackend, frame: &Frame<'_>) -> Result<bool> {
    backend.present_frame(frame)
}

pub fn viewport_size(backend: &dyn PresentationBackend) -> [u32; 2] {
    backend.viewport_size()
}

pub fn resize(backend: &mut dyn PresentationBackend, width: u32, height: u32) {
    backend.resize(width, height);
}

pub fn release_surface(backend: &mut dyn PresentationBackend) {
    backend.release_surface();
}

pub fn recreate_surface(backend: &mut dyn PresentationBackend) -> Result<()> {
    backend.recreate_surface()
}

pub fn set_present_mode(backend: &mut dyn PresentationBackend, vsync: bool, mode: PresentMode) {
    backend.set_present_mode(vsync, mode);
}

pub fn set_gamma(backend: &mut dyn PresentationBackend, gamma: f32) {
    backend.set_gamma(gamma);
}

pub fn set_hud_sprite_animations(
    backend: &mut dyn HudBackend,
    animations: &[(Rect, SpriteAnimation)],
) {
    backend.set_hud_sprite_animations(animations);
}

pub fn set_hud_text_shader(
    backend: &mut dyn HudBackend,
    shader: Option<&horizon_shaders::CoreProgram>,
) {
    backend.set_hud_text_shader(shader);
}

pub fn set_hud_cit_glints(backend: &mut dyn HudBackend, textures: Option<&GlintTextures>) {
    backend.set_hud_cit_glints(textures);
}

pub fn set_lines_shader(
    backend: &mut dyn HudBackend,
    shader: Option<&horizon_shaders::CoreProgram>,
) {
    backend.set_lines_shader(shader);
}

pub fn max_texture_dimension_2d(backend: &dyn HudBackend) -> u32 {
    backend.max_texture_dimension_2d()
}

/// Upload one worker-produced section mesh through the selected terrain backend.
pub fn upload_section(
    backend: &mut dyn TerrainBackend,
    key: SectionKey,
    mesh: &SectionMesh,
) -> Result<()> {
    backend.upload_section(key, mesh)
}

pub fn clear_sections(backend: &mut dyn TerrainBackend, layout: ColumnLayout) {
    backend.clear_sections(layout);
}

pub fn remove_columns(
    backend: &mut dyn TerrainBackend,
    columns: impl IntoIterator<Item = (i32, i32)>,
) {
    backend.remove_columns(&mut columns.into_iter());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesher::SectionMesh;
    use std::collections::HashSet;

    #[derive(Default)]
    struct FakeTerrain {
        sections: HashSet<SectionKey>,
        fail_next_upload: bool,
    }

    impl TerrainBackend for FakeTerrain {
        fn upload_section(&mut self, key: SectionKey, _: &SectionMesh) -> Result<()> {
            if std::mem::take(&mut self.fail_next_upload) {
                return Err("simulated upload failure".into());
            }
            self.sections.insert(key);
            Ok(())
        }

        fn clear_sections(&mut self, _: ColumnLayout) {
            self.sections.clear();
        }

        fn remove_columns(&mut self, columns: &mut dyn Iterator<Item = (i32, i32)>) {
            let removed: HashSet<_> = columns.collect();
            self.sections
                .retain(|(x, _, z)| !removed.contains(&(*x, *z)));
        }
    }

    #[test]
    fn failed_upload_is_reported_and_does_not_install_the_section() {
        let mut backend = FakeTerrain {
            fail_next_upload: true,
            ..Default::default()
        };
        let mesh = SectionMesh::default();

        assert!(upload_section(&mut backend, (1, 0, 2), &mesh).is_err());
        assert!(!backend.sections.contains(&(1, 0, 2)));

        upload_section(&mut backend, (1, 0, 2), &mesh).unwrap();
        assert!(backend.sections.contains(&(1, 0, 2)));
    }

    #[test]
    fn section_lifecycle_routes_replacement_removal_and_clear() {
        let mut backend = FakeTerrain::default();
        let mesh = SectionMesh::default();

        upload_section(&mut backend, (1, 0, 2), &mesh).unwrap();
        upload_section(&mut backend, (1, 1, 2), &mesh).unwrap();
        upload_section(&mut backend, (3, 0, 4), &mesh).unwrap();
        assert_eq!(backend.sections.len(), 3);

        remove_columns(&mut backend, [(1, 2)]);
        assert!(!backend.sections.contains(&(1, 0, 2)));
        assert!(!backend.sections.contains(&(1, 1, 2)));
        assert!(backend.sections.contains(&(3, 0, 4)));

        clear_sections(&mut backend, ColumnLayout::VANILLA);
        assert!(backend.sections.is_empty());
    }

    #[derive(Default)]
    struct FakeSurface {
        size: [u32; 2],
        released: bool,
        present_mode: Option<(bool, PresentMode)>,
        gamma: f32,
    }

    impl PresentationBackend for FakeSurface {
        fn viewport_size(&self) -> [u32; 2] {
            self.size
        }

        fn present_frame(&mut self, _: &Frame<'_>) -> Result<bool> {
            Ok(true)
        }

        fn resize(&mut self, width: u32, height: u32) {
            self.size = [width, height];
        }

        fn release_surface(&mut self) {
            self.released = true;
        }

        fn recreate_surface(&mut self) -> Result<()> {
            self.released = false;
            Ok(())
        }

        fn set_present_mode(&mut self, vsync: bool, mode: PresentMode) {
            self.present_mode = Some((vsync, mode));
        }

        fn set_gamma(&mut self, gamma: f32) {
            self.gamma = gamma;
        }
    }

    #[test]
    fn surface_lifecycle_can_be_driven_through_the_contract() {
        let mut backend = FakeSurface::default();
        backend.resize(1280, 720);
        assert_eq!(viewport_size(&backend), [1280, 720]);
        assert_eq!(backend.size, [1280, 720]);

        backend.release_surface();
        assert!(backend.released);
        backend.recreate_surface().unwrap();
        assert!(!backend.released);
        set_gamma(&mut backend, 0.5);
        assert_eq!(backend.gamma, 0.5);
    }
}
