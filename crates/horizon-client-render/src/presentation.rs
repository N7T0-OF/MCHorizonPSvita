//! Backend contracts between Horizon's CPU-side frame/section data and a
//! window-facing renderer.
//!
//! The desktop renderer is one implementation. Platform renderers can consume
//! the same [`Frame`] and [`SectionMesh`] without changing world streaming or
//! frame construction.

use crate::{
    Result,
    chunk::ColumnLayout,
    gfx::{Frame, Renderer, SectionKey},
    graphics::PresentMode,
    mesher::SectionMesh,
};

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
}

impl PresentationBackend for Renderer {
    fn viewport_size(&self) -> [u32; 2] {
        [self.config.width, self.config.height]
    }

    fn present_frame(&mut self, frame: &Frame<'_>) -> Result<bool> {
        self.render(frame)
    }

    fn resize(&mut self, width: u32, height: u32) {
        Renderer::resize(self, width, height);
    }

    fn release_surface(&mut self) {
        Renderer::release_surface(self);
    }

    fn recreate_surface(&mut self) -> Result<()> {
        Renderer::recreate_surface(self)
    }

    fn set_present_mode(&mut self, vsync: bool, mode: PresentMode) {
        Renderer::set_present_mode(self, vsync, mode);
    }
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

impl TerrainBackend for Renderer {
    fn upload_section(&mut self, key: SectionKey, mesh: &SectionMesh) -> Result<()> {
        Renderer::upload(self, key, mesh)
    }

    fn clear_sections(&mut self, layout: ColumnLayout) {
        Renderer::clear_sections(self, layout);
    }

    fn remove_columns(&mut self, columns: &mut dyn Iterator<Item = (i32, i32)>) {
        Renderer::remove_columns(self, columns);
    }
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
    }
}
