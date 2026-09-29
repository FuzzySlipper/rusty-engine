//! Bind group layouts and the fixed set of render pipelines, one set per
//! target colour format (offscreen RGBA8 sRGB, and whatever a surface uses).

use crate::target::DEPTH_FORMAT;

/// Interleaved position (3), normal (3), uv (2).
pub(crate) const VERTEX_FLOATS: usize = 8;

pub(crate) struct Layouts {
    pub frame: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
    pub sky: wgpu::BindGroupLayout,
    world: wgpu::PipelineLayout,
    sky_pipeline: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
}

pub(crate) struct Pipelines {
    pub format: wgpu::TextureFormat,
    pub opaque: wgpu::RenderPipeline,
    pub opaque_double_sided: wgpu::RenderPipeline,
    pub blend: wgpu::RenderPipeline,
    pub blend_double_sided: wgpu::RenderPipeline,
    pub lines: wgpu::RenderPipeline,
    pub sky: wgpu::RenderPipeline,
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

impl Layouts {
    pub fn new(device: &wgpu::Device) -> Self {
        let frame = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu frame"),
            entries: &[uniform_entry(0), storage_entry(1), storage_entry(2)],
        });
        let material = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu material"),
            entries: &[uniform_entry(0), texture_entry(1), sampler_entry(2)],
        });
        let sky = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu sky"),
            entries: &[
                uniform_entry(0),
                texture_entry(1),
                texture_entry(2),
                sampler_entry(3),
                sampler_entry(4),
            ],
        });
        let world = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu world"),
            bind_group_layouts: &[Some(&frame), Some(&material)],
            immediate_size: 0,
        });
        let sky_pipeline = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu sky"),
            bind_group_layouts: &[Some(&frame), Some(&sky)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render-wgpu world"),
            source: wgpu::ShaderSource::Wgsl(include_str!("world.wgsl").into()),
        });
        Self {
            frame,
            material,
            sky,
            world,
            sky_pipeline,
            shader,
        }
    }

    pub fn pipelines(&self, device: &wgpu::Device, format: wgpu::TextureFormat) -> Pipelines {
        let world = |label, topology, cull_mode, blend: bool| {
            let attributes =
                wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&self.world),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_world"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: (VERTEX_FLOATS * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    cull_mode,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(!blend),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_world"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: blend.then_some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let triangles = wgpu::PrimitiveTopology::TriangleList;
        let back = Some(wgpu::Face::Back);
        let sky = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu sky"),
            layout: Some(&self.sky_pipeline),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_sky"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        Pipelines {
            format,
            opaque: world("render-wgpu opaque", triangles, back, false),
            opaque_double_sided: world("render-wgpu opaque double-sided", triangles, None, false),
            blend: world("render-wgpu blend", triangles, back, true),
            blend_double_sided: world("render-wgpu blend double-sided", triangles, None, true),
            lines: world(
                "render-wgpu lines",
                wgpu::PrimitiveTopology::LineList,
                None,
                false,
            ),
            sky,
        }
    }
}
