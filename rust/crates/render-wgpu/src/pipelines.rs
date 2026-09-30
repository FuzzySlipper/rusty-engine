//! Bind group layouts and the fixed set of render pipelines, one set per
//! target colour format (offscreen RGBA8 sRGB, and whatever a surface uses).

use crate::batch::Pass;
use crate::target::{ColorTarget, DEPTH_FORMAT};

/// Interleaved position (3), normal (3), uv (2), linear RGBA colour (4;
/// white unless a static mesh supplies vertex colours).
pub(crate) const VERTEX_FLOATS: usize = 12;

pub(crate) struct Layouts {
    pub frame: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
    pub sky: wgpu::BindGroupLayout,
    /// Shadow caster pass: parts, instances, shadow views.
    pub casters: wgpu::BindGroupLayout,
    /// Shadow caster pass: the layer index (dynamic offset).
    pub shadow_layer: wgpu::BindGroupLayout,
    world: wgpu::PipelineLayout,
    sky_pipeline: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    pub shadow: ShadowPipelines,
}

pub(crate) struct Pipelines {
    pub target: ColorTarget,
    pub opaque: wgpu::RenderPipeline,
    pub opaque_mirrored: wgpu::RenderPipeline,
    pub opaque_double_sided: wgpu::RenderPipeline,
    pub blend: wgpu::RenderPipeline,
    pub blend_mirrored: wgpu::RenderPipeline,
    pub blend_double_sided: wgpu::RenderPipeline,
    pub lines: wgpu::RenderPipeline,
    pub sky: wgpu::RenderPipeline,
}

impl Pipelines {
    pub fn get(&self, pass: Pass) -> &wgpu::RenderPipeline {
        match pass {
            Pass::Opaque => &self.opaque,
            Pass::OpaqueMirrored => &self.opaque_mirrored,
            Pass::OpaqueDoubleSided => &self.opaque_double_sided,
            Pass::Lines => &self.lines,
            Pass::Blend => &self.blend,
            Pass::BlendMirrored => &self.blend_mirrored,
            Pass::BlendDoubleSided => &self.blend_double_sided,
        }
    }
}

/// Depth-only caster pipelines, by face culling: single-sided parts render
/// their back faces, mirrored parts wind the
/// other way, double-sided parts render both.
pub(crate) struct ShadowPipelines {
    pub back_faces: wgpu::RenderPipeline,
    pub back_faces_mirrored: wgpu::RenderPipeline,
    pub both_faces: wgpu::RenderPipeline,
}

impl ShadowPipelines {
    /// Caster lists only hold the opaque passes.
    pub fn get(&self, pass: Pass) -> &wgpu::RenderPipeline {
        match pass {
            Pass::OpaqueMirrored | Pass::BlendMirrored => &self.back_faces_mirrored,
            Pass::OpaqueDoubleSided | Pass::BlendDoubleSided => &self.both_faces,
            Pass::Opaque | Pass::Blend | Pass::Lines => &self.back_faces,
        }
    }
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

fn depth_array_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
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
            entries: &[
                uniform_entry(0),
                storage_entry(1),
                storage_entry(2),
                storage_entry(3),
                depth_array_entry(4),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                storage_entry(6),
            ],
        });
        let casters = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu casters"),
            entries: &[storage_entry(0), storage_entry(1), storage_entry(2)],
        });
        let shadow_layer = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu shadow layer"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(16),
                },
                count: None,
            }],
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
        let shadow = ShadowPipelines::new(device, &casters, &material, &shadow_layer);
        Self {
            frame,
            material,
            sky,
            casters,
            shadow_layer,
            world,
            sky_pipeline,
            shader,
            shadow,
        }
    }

    pub fn pipelines(&self, device: &wgpu::Device, target: ColorTarget) -> Pipelines {
        let format = target.format;
        let world = |label, topology, cull_mode, front_face, blend: bool| {
            let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
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
                    front_face,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(!blend),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: target.multisample(),
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
            multisample: target.multisample(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let (ccw, cw) = (wgpu::FrontFace::Ccw, wgpu::FrontFace::Cw);
        Pipelines {
            target,
            opaque: world("render-wgpu opaque", triangles, back, ccw, false),
            opaque_mirrored: world("render-wgpu opaque mirrored", triangles, back, cw, false),
            opaque_double_sided: world(
                "render-wgpu opaque double-sided",
                triangles,
                None,
                ccw,
                false,
            ),
            blend: world("render-wgpu blend", triangles, back, ccw, true),
            blend_mirrored: world("render-wgpu blend mirrored", triangles, back, cw, true),
            blend_double_sided: world("render-wgpu blend double-sided", triangles, None, ccw, true),
            lines: world(
                "render-wgpu lines",
                wgpu::PrimitiveTopology::LineList,
                None,
                ccw,
                false,
            ),
            sky,
        }
    }
}

impl ShadowPipelines {
    fn new(
        device: &wgpu::Device,
        casters: &wgpu::BindGroupLayout,
        material: &wgpu::BindGroupLayout,
        layer: &wgpu::BindGroupLayout,
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu shadow"),
            bind_group_layouts: &[Some(casters), Some(material), Some(layer)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render-wgpu shadow"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shadow.wgsl").into()),
        });
        let pipeline = |label, cull_mode| {
            let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_shadow"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: (VERTEX_FLOATS * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_shadow"),
                    compilation_options: Default::default(),
                    targets: &[],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            back_faces: pipeline("render-wgpu shadow back faces", Some(wgpu::Face::Front)),
            back_faces_mirrored: pipeline(
                "render-wgpu shadow back faces mirrored",
                Some(wgpu::Face::Back),
            ),
            both_faces: pipeline("render-wgpu shadow both faces", None),
        }
    }
}
