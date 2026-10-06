//! Bind group layouts and the render pipelines: per target colour format
//! (offscreen RGBA8 sRGB, and whatever a surface uses), the sky and a world
//! pipeline per material feature set and pass; depth-only shadow casters per
//! caster feature set and culling. Pipelines are made when a material is
//! defined and before a pass is encoded, never while drawing.

use std::collections::HashMap;

use crate::batch::Pass;
use crate::shaders::{Entry, Features, Shaders};
use crate::target::{ColorTarget, DEPTH_FORMAT};

/// Interleaved position (3), normal (3), uv (2), linear RGBA colour (4;
/// white unless a static mesh supplies vertex colours).
pub const VERTEX_FLOATS: usize = 12;

/// The optional second stream (`GpuMesh::extra`): tangent (4), uv1 (2).
pub const EXTRA_VERTEX_FLOATS: usize = 6;

/// A standard-family module, which always composes.
pub(crate) fn standard(module: Result<wgpu::ShaderModule, String>) -> wgpu::ShaderModule {
    module.unwrap_or_else(|error| panic!("{error}"))
}

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
    shadow_pipeline: wgpu::PipelineLayout,
    pub shaders: Shaders,
    sky_shader: wgpu::ShaderModule,
    /// The world entry compiled per feature set in use.
    world_shaders: HashMap<Features, wgpu::ShaderModule>,
    /// The caster entry per caster feature set (`Features::caster`).
    shadow_shaders: HashMap<Features, wgpu::ShaderModule>,
    pub shadow: ShadowPipelines,
    /// Clears one shadow tile's depth to the far plane within its viewport.
    pub shadow_clear: wgpu::RenderPipeline,
    /// Product shaders that did not compose since last taken; their
    /// materials draw with the standard shade stage.
    pub shader_errors: Vec<String>,
}

/// One target format's pipelines: the sky, and a world pipeline per feature
/// set and pass, created as materials need them.
pub(crate) struct Pipelines {
    pub target: ColorTarget,
    pub sky: wgpu::RenderPipeline,
    world: HashMap<(Features, Pass), wgpu::RenderPipeline>,
}

impl Pipelines {
    /// A pipeline `Layouts::prepare` made before the pass was encoded.
    pub fn get(&self, pass: Pass, features: Features) -> &wgpu::RenderPipeline {
        &self.world[&(features, pass)]
    }
}

/// The caster pass's slope-scaled depth bias, in depth per unit of the
/// face's depth slope.
const CASTER_SLOPE_BIAS: f32 = 1.0;

/// Depth-only caster pipelines by caster features and face culling:
/// single-sided parts render their back faces, mirrored parts wind the other
/// way, double-sided parts render both.
#[derive(Default)]
pub(crate) struct ShadowPipelines {
    pipelines: HashMap<(Features, Option<wgpu::Face>), wgpu::RenderPipeline>,
}

impl ShadowPipelines {
    /// Caster lists only hold the opaque passes.
    fn cull(pass: Pass) -> Option<wgpu::Face> {
        match pass {
            Pass::OpaqueMirrored | Pass::BlendMirrored => Some(wgpu::Face::Back),
            Pass::OpaqueDoubleSided | Pass::BlendDoubleSided => None,
            Pass::Opaque | Pass::Blend | Pass::Lines => Some(wgpu::Face::Front),
        }
    }

    /// A pipeline `Layouts::prepare_caster` made before the pass was encoded.
    pub fn get(&self, pass: Pass, features: Features) -> &wgpu::RenderPipeline {
        &self.pipelines[&(features.caster(), Self::cull(pass))]
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
            // The frame (a product caster's time), parts, instances and shadow
            // views at their `rusty::view` numbers.
            entries: &[
                uniform_entry(0),
                storage_entry(1),
                storage_entry(3),
                storage_entry(6),
            ],
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
            // Uniform, albedo; then emissive, normal and occlusion maps; then
            // a product shader's two maps; then terrain layers 1 to 3's base
            // textures and normal maps.
            entries: &[
                uniform_entry(0),
                texture_entry(1),
                sampler_entry(2),
                texture_entry(3),
                sampler_entry(4),
                texture_entry(5),
                sampler_entry(6),
                texture_entry(7),
                sampler_entry(8),
                texture_entry(9),
                sampler_entry(10),
                texture_entry(11),
                sampler_entry(12),
                texture_entry(13),
                texture_entry(14),
                texture_entry(15),
                texture_entry(16),
                texture_entry(17),
                texture_entry(18),
            ],
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
        let shadow_pipeline = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu shadow"),
            bind_group_layouts: &[Some(&casters), Some(&material), Some(&shadow_layer)],
            immediate_size: 0,
        });
        let mut shaders = Shaders::new();
        let sky_shader = standard(shaders.module(device, Entry::Sky, Features::default()));
        let compose = standard(shaders.module(device, Entry::Compose, Features::default()));
        let shadow_clear = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu shadow tile clear"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("render-wgpu shadow tile clear"),
                    bind_group_layouts: &[],
                    immediate_size: 0,
                }),
            ),
            vertex: wgpu::VertexState {
                module: &compose,
                entry_point: Some("vs_fullscreen"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        Self {
            frame,
            material,
            sky,
            casters,
            shadow_layer,
            world,
            sky_pipeline,
            shadow_pipeline,
            shaders,
            sky_shader,
            world_shaders: HashMap::new(),
            shadow_shaders: HashMap::new(),
            shadow: ShadowPipelines::default(),
            shadow_clear,
            shader_errors: Vec::new(),
        }
    }

    /// The world entry compiled with `features`, compiled now if new. A
    /// product shader that does not compose is compiled with the standard
    /// shade stage instead, and its error kept in `shader_errors`.
    pub fn world_shader(
        &mut self,
        device: &wgpu::Device,
        features: Features,
    ) -> &wgpu::ShaderModule {
        self.world_shaders.entry(features).or_insert_with(|| {
            self.shaders
                .module(device, Entry::World, features)
                .unwrap_or_else(|error| {
                    self.shader_errors.push(error);
                    standard(
                        self.shaders
                            .module(device, Entry::World, features.with_product(0)),
                    )
                })
        })
    }

    /// Feature sets compiled for the world and caster passes.
    pub fn shader_variants(&self) -> usize {
        self.world_shaders.len() + self.shadow_shaders.len()
    }

    /// A target's pipeline set, before any world pipeline is prepared.
    pub fn pipelines(&self, device: &wgpu::Device, target: ColorTarget) -> Pipelines {
        let sky = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu sky"),
            layout: Some(&self.sky_pipeline),
            vertex: wgpu::VertexState {
                module: &self.sky_shader,
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
                module: &self.sky_shader,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &[Some(target.format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        Pipelines {
            target,
            sky,
            world: HashMap::new(),
        }
    }

    /// Make the world pipeline drawing `features` in `pass` on `pipelines`'
    /// target, compiling the feature set's shader if it is new. Returns
    /// whether it was made now.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        pipelines: &mut Pipelines,
        features: Features,
        pass: Pass,
    ) -> bool {
        if pipelines.world.contains_key(&(features, pass)) {
            return false;
        }
        self.world_shader(device, features);
        let shader = &self.world_shaders[&features];
        let (topology, cull_mode, front_face, blend) = match pass {
            Pass::Opaque => (TRIANGLES, BACK, CCW, false),
            Pass::OpaqueMirrored => (TRIANGLES, BACK, CW, false),
            Pass::OpaqueDoubleSided => (TRIANGLES, None, CCW, false),
            Pass::Lines => (wgpu::PrimitiveTopology::LineList, None, CCW, false),
            Pass::Blend => (TRIANGLES, BACK, CCW, true),
            Pass::BlendMirrored => (TRIANGLES, BACK, CW, true),
            Pass::BlendDoubleSided => (TRIANGLES, None, CCW, true),
        };
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
        let extra_attributes = wgpu::vertex_attr_array![4 => Float32x4, 5 => Float32x2];
        let buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: (VERTEX_FLOATS * 4) as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: (EXTRA_VERTEX_FLOATS * 4) as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &extra_attributes,
            }),
        ];
        let streams = if features.contains(Features::VERTEX_TANGENTS) {
            2
        } else {
            1
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu world"),
            layout: Some(&self.world),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_world"),
                compilation_options: Default::default(),
                buffers: &buffers[..streams],
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
            multisample: pipelines.target.multisample(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_world"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: pipelines.target.format,
                    blend: blend.then_some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        pipelines.world.insert((features, pass), pipeline);
        true
    }

    /// Make the caster pipeline drawing `features` in `pass` (opaque passes
    /// only select the culling). Casters without an alpha mask write depth
    /// with no fragment stage. Returns whether it was made now.
    pub fn prepare_caster(
        &mut self,
        device: &wgpu::Device,
        features: Features,
        pass: Pass,
    ) -> bool {
        let (features, cull_mode) = (features.caster(), ShadowPipelines::cull(pass));
        if self.shadow.pipelines.contains_key(&(features, cull_mode)) {
            return false;
        }
        let shader = self
            .shadow_shaders
            .entry(features)
            .or_insert_with(|| standard(self.shaders.module(device, Entry::Shadow, features)));
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu shadow"),
            layout: Some(&self.shadow_pipeline),
            vertex: wgpu::VertexState {
                module: shader,
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
                // Steeper faces, whose depth changes most across a texel,
                // cast further back. Receivers also look up from along their
                // normal (`rusty::lighting::shadow_lookup`).
                bias: wgpu::DepthBiasState {
                    constant: 0,
                    slope_scale: CASTER_SLOPE_BIAS,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            // Only to discard: the alpha mask or a product's caster stage.
            fragment: (features.contains(Features::MASK) || features.product() != 0).then(|| {
                wgpu::FragmentState {
                    module: shader,
                    entry_point: Some("fs_shadow"),
                    compilation_options: Default::default(),
                    targets: &[],
                }
            }),
            multiview_mask: None,
            cache: None,
        });
        self.shadow
            .pipelines
            .insert((features, cull_mode), pipeline);
        true
    }
}

const TRIANGLES: wgpu::PrimitiveTopology = wgpu::PrimitiveTopology::TriangleList;
const BACK: Option<wgpu::Face> = Some(wgpu::Face::Back);
const CCW: wgpu::FrontFace = wgpu::FrontFace::Ccw;
const CW: wgpu::FrontFace = wgpu::FrontFace::Cw;
