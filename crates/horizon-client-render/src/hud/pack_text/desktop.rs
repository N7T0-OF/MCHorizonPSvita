//! WGPU pipeline for the resource-pack GUI text shader.
use super::uniforms;
use crate::hud::{HudBatch, HudVertex};
use horizon_shaders::CoreProgram;

/// Each uniform block's offset in the one buffer (`core_shader::BLOCKS` order).
const BLOCK_OFFSETS: [u64; 4] = [0, 256, 512, 768];
const BLOCK_SIZES: [u64; 4] = [48, 64, 160, 48];

/// The pipeline and its resources.
pub(in crate::hud) struct PackText {
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
}

impl PackText {
    pub(in crate::hud) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        atlas: &wgpu::TextureView,
        program: &CoreProgram,
    ) -> Option<Self> {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = |label, module: &naga::Module| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module.clone())),
            })
        };
        let vertex = module("Resource pack text (vertex)", &program.vertex);
        let fragment = module("Resource pack text (fragment)", &program.fragment);
        let visibility = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let mut entries: Vec<wgpu::BindGroupLayoutEntry> = (0..4)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();
        for (_, binding) in horizon_shaders::SAMPLERS {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: binding + 1,
                visibility,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Resource pack text"),
            entries: &entries,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Resource pack text uniforms"),
            size: 1024,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // The font textures' nearest filtering; every sampler a pack names reads the atlas.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Resource pack text"),
            ..Default::default()
        });
        let mut resources: Vec<wgpu::BindGroupEntry> = (0..4)
            .map(|i| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform,
                    offset: BLOCK_OFFSETS[i],
                    size: wgpu::BufferSize::new(BLOCK_SIZES[i]),
                }),
            })
            .collect();
        for (_, binding) in horizon_shaders::SAMPLERS {
            resources.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(atlas),
            });
            resources.push(wgpu::BindGroupEntry {
                binding: binding + 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            });
        }
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Resource pack text"),
            layout: &layout,
            entries: &resources,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Resource pack text"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let attributes = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Unorm8x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Resource pack text"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex,
                entry_point: Some("main"),
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
                module: &fragment,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                // BlendFunction.TRANSLUCENT.
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        if let Some(e) = pollster::block_on(scope.pop()) {
            log::warn!("the resource pack's text shader is not used: {e}");
            return None;
        }
        Some(Self {
            pipeline,
            group,
            uniform,
        })
    }

    /// The frame's blocks, as `GuiRenderer` sets them for the GUI.
    pub(in crate::hud) fn prepare(
        &self,
        uploads: &mut crate::gfx::upload::Uploads,
        encoder: &mut wgpu::CommandEncoder,
        batch: &HudBatch,
        size: [u32; 2],
        scale: f32,
    ) {
        uploads.buffer(encoder, &self.uniform, 0, &uniforms(batch, size, scale));
    }

    /// Draw `range` (whole glyph quads) with the pack's shader; the caller sets its own
    /// pipeline and bind group again after.
    pub(in crate::hud) fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        range: std::ops::Range<u32>,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.draw(range.clone(), range.start..range.start + 1);
    }
}
