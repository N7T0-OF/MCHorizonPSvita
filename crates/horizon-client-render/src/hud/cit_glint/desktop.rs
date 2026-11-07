//! WGPU pipeline for Horizon's CIT glints.
use super::{GlintTextures, MAX_SLOTS};
use crate::hud::HudVertex;
use horizon_optifine::cit::{Blend, Factor};

fn factor(f: Factor) -> wgpu::BlendFactor {
    use wgpu::BlendFactor as W;
    match f {
        Factor::Zero => W::Zero,
        Factor::One => W::One,
        Factor::SrcColor => W::Src,
        Factor::OneMinusSrcColor => W::OneMinusSrc,
        Factor::SrcAlpha => W::SrcAlpha,
        Factor::OneMinusSrcAlpha => W::OneMinusSrcAlpha,
        Factor::DstAlpha => W::DstAlpha,
        Factor::OneMinusDstAlpha => W::OneMinusDstAlpha,
        Factor::DstColor => W::Dst,
        Factor::OneMinusDstColor => W::OneMinusDst,
        Factor::SrcAlphaSaturate => W::SrcAlphaSaturated,
    }
}

/// `glBlendFuncSeparate` with the rule's factors; `replace` draws unblended.
pub fn blend_state(blend: Option<Blend>) -> wgpu::BlendState {
    let Some(b) = blend else {
        return wgpu::BlendState::REPLACE;
    };
    let component = |src, dst| wgpu::BlendComponent {
        src_factor: factor(src),
        dst_factor: factor(dst),
        operation: wgpu::BlendOperation::Add,
    };
    wgpu::BlendState {
        color: component(b.src, b.dst),
        alpha: component(b.src_alpha, b.dst_alpha),
    }
}

/// The GPU side: the texture array and slot table (bind group 1 of `fs_cit_glint`), and one
/// pipeline per blend.
pub(in crate::hud) struct CitGlintPass {
    group: wgpu::BindGroup,
    pipelines: Vec<wgpu::RenderPipeline>,
}

impl CitGlintPass {
    /// `None` without textures. `hud` is the HUD's bind group layout (group 0).
    pub(in crate::hud) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        hud: &wgpu::BindGroupLayout,
        textures: &GlintTextures,
    ) -> Option<Self> {
        use wgpu::util::DeviceExt;
        if textures.is_empty() {
            return None;
        }
        // wgpu's OpenGL backend makes 1 layer a 2D texture and 6n a cube map
        // (`gfx::array_layers`): pad with empty layers, within the GPU's limit. Slots past the
        // layers kept sample the last one.
        let gl = device.adapter_info().backend == wgpu::Backend::Gl;
        let max = device.limits().max_texture_array_layers.max(1);
        let slots = textures.slots.len() as u32;
        let mut kept = slots.min(max);
        while kept > 1 && crate::gfx::array_layers(kept, gl) > max {
            kept -= 1;
        }
        if kept < slots {
            log::warn!("custom item textures: the GPU holds {kept} of {slots} glint textures");
        }
        let layers = crate::gfx::array_layers(kept, gl).min(max);
        let layer_bytes = textures.width as usize * textures.height as usize * 4;
        let mut data = std::borrow::Cow::Borrowed(&textures.pixels[..]);
        if layers != slots {
            let mut padded = textures.pixels[..layer_bytes * kept as usize].to_vec();
            padded.resize(layer_bytes * layers as usize, 0);
            data = std::borrow::Cow::Owned(padded);
        }
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("CIT glints"),
                size: wgpu::Extent3d {
                    width: textures.width,
                    height: textures.height,
                    depth_or_array_layers: layers,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &data,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = |label, filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        let linear = sampler("CIT glint blur", wgpu::FilterMode::Linear);
        let nearest = sampler("CIT glint", wgpu::FilterMode::Nearest);
        // hud.wgsl's `CitSlot`: rgba, then blur in x.
        let mut table = vec![[0.0f32; 8]; MAX_SLOTS];
        for (row, slot) in table.iter_mut().zip(&textures.slots) {
            row[..4].copy_from_slice(&slot.rgba);
            row[4] = if slot.blur { 1.0 } else { 0.0 };
        }
        let table = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CIT glint slots"),
            contents: bytemuck::cast_slice(&table),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let fragment = wgpu::ShaderStages::FRAGMENT;
        let filtering = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: fragment,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CIT glint bindings"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                ),
                entry(1, filtering),
                entry(2, filtering),
                entry(
                    3,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
            ],
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CIT glint resources"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&linear),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&nearest),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: table.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Horizon HUD (CIT glints)"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../hud.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("CIT glint layout"),
            bind_group_layouts: &[Some(hud), Some(&layout)],
            ..Default::default()
        });
        let attributes =
            wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Unorm8x4,3=>Float32,4=>Float32x2];
        let pipelines = textures
            .blends
            .iter()
            .map(|&blend| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("CIT glint"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[Some(wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<HudVertex>() as u64,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &attributes,
                        })],
                    },
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_cit_glint"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(blend_state(blend)),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
            })
            .collect();
        Some(Self { group, pipelines })
    }

    /// Draw `vertices` with blend `blend` (an index into `GlintTextures::blends`).
    pub(in crate::hud) fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        blend: u8,
        vertices: std::ops::Range<u32>,
    ) {
        if let Some(pipeline) = self.pipelines.get(usize::from(blend)) {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(1, &self.group, &[]);
            pass.draw(vertices, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::blend_state;
    use horizon_optifine::cit::parse_blend;

    #[test]
    fn named_blends_take_their_gl_factors() {
        let multiply = blend_state(parse_blend("multiply").unwrap());
        assert_eq!(multiply.color.src_factor, wgpu::BlendFactor::Dst);
        assert_eq!(
            multiply.color.dst_factor,
            wgpu::BlendFactor::OneMinusSrcAlpha
        );
        // The alpha factors default to GL_ZERO, GL_ONE: the target's alpha stays.
        assert_eq!(multiply.alpha.src_factor, wgpu::BlendFactor::Zero);
        assert_eq!(multiply.alpha.dst_factor, wgpu::BlendFactor::One);
        assert_eq!(
            blend_state(parse_blend("replace").unwrap()),
            wgpu::BlendState::REPLACE
        );
    }
}
