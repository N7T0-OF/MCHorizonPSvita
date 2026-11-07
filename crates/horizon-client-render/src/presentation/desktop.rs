//! Desktop WGPU implementation of Horizon's presentation and terrain contracts.

use super::{HudBackend, PresentationBackend, TerrainBackend};
use crate::{
    Result,
    chunk::ColumnLayout,
    gfx::{Frame, Renderer, SectionKey},
    graphics::PresentMode,
    hud::{Rect, cit_glint::GlintTextures},
    mesher::SectionMesh,
    sprites::SpriteAnimation,
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

    fn set_gamma(&mut self, gamma: f32) {
        self.gamma = gamma;
    }
}

impl HudBackend for Renderer {
    fn set_hud_sprite_animations(&mut self, animations: &[(Rect, SpriteAnimation)]) {
        self.hud.sprite_animations = animations
            .iter()
            .map(|(rect, animation)| (*rect, animation.clone(), (usize::MAX, 0, 0)))
            .collect();
    }

    fn set_hud_text_shader(&mut self, shader: Option<&horizon_shaders::CoreProgram>) {
        self.hud.set_text_shader(&self.device, shader);
    }

    fn set_hud_cit_glints(&mut self, textures: Option<&GlintTextures>) {
        self.hud.set_cit_glints(&self.device, &self.queue, textures);
    }

    fn set_lines_shader(&mut self, shader: Option<&horizon_shaders::CoreProgram>) {
        Renderer::set_lines_shader(self, shader);
    }

    fn max_texture_dimension_2d(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
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
