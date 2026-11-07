//! Desktop WGPU implementation of Horizon's presentation and terrain contracts.

use super::{PresentationBackend, TerrainBackend};
use crate::{
    Result,
    chunk::ColumnLayout,
    gfx::{Frame, Renderer, SectionKey},
    graphics::PresentMode,
    mesher::SectionMesh,
};

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
