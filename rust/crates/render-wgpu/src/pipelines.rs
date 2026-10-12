//! Bind group layouts and the render pipelines: per target colour format
//! (offscreen RGBA8 sRGB, and whatever a surface uses), the sky and a world
//! pipeline per material feature set and pass; depth-only shadow casters per
//! caster feature set and culling, and the ambient occlusion pre-pass's
//! depth-only pipelines per caster feature set and world pass (the caster
//! shaders with the world pass's culling). Pipelines are made when a material
//! is defined and before a pass is encoded, never while drawing.

use std::collections::HashMap;

use crate::batch::Pass;
use crate::shaders::{Entry, Features, Shaders};
use crate::target::{ColorTarget, DEPTH_FORMAT};

/// Interleaved position (3), normal (3), uv (2), linear RGBA colour (4;
/// white unless a static mesh supplies vertex colours).
pub const VERTEX_FLOATS: usize = 12;

/// The optional second stream (`GpuMesh::extra`): tangent (4), uv1 (2).
pub const EXTRA_VERTEX_FLOATS: usize = 6;

/// A morphing mesh's second stream instead (`GpuMesh::morph`): height
/// change, target normal x and z.
pub const MORPH_VERTEX_FLOATS: usize = 3;

const MESH_ATTRIBUTES: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
const EXTRA_ATTRIBUTES: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![4 => Float32x4, 5 => Float32x2];
const MORPH_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![6 => Float32x3];

/// The vertex streams a mesh with `features` binds: the interleaved one,
/// then its tangents (VERTEX_TANGENTS) or its morph targets (MORPH). The
/// shadow and pre-pass pipelines take tangents never.
fn mesh_buffers(
    features: Features,
    tangents: bool,
) -> Vec<Option<wgpu::VertexBufferLayout<'static>>> {
    let mut buffers = vec![Some(wgpu::VertexBufferLayout {
        array_stride: (VERTEX_FLOATS * 4) as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &MESH_ATTRIBUTES,
    })];
    if tangents && features.contains(Features::VERTEX_TANGENTS) {
        buffers.push(Some(wgpu::VertexBufferLayout {
            array_stride: (EXTRA_VERTEX_FLOATS * 4) as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &EXTRA_ATTRIBUTES,
        }));
    } else if features.contains(Features::MORPH) {
        buffers.push(Some(wgpu::VertexBufferLayout {
            array_stride: (MORPH_VERTEX_FLOATS * 4) as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &MORPH_ATTRIBUTES,
        }));
    }
    buffers
}

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
    /// The world layout for a water material: its group 2 is the opaque
    /// depth copy (`water.rs`) rather than the view's occlusion.
    water: wgpu::PipelineLayout,
    sky_pipeline: wgpu::PipelineLayout,
    /// The volumetric clouds' reduced march (`cloud_march.rs`): its group 2
    /// holds the view's history, and its pipeline draws into the reduced
    /// target, whatever the view's own.
    pub cloud_march: wgpu::BindGroupLayout,
    pub cloud_march_pipeline: wgpu::RenderPipeline,
    /// The composite of the reduced clouds over a view's background: its
    /// group 2 holds the reduced target.
    pub cloud_composite: wgpu::BindGroupLayout,
    cloud_composite_pipeline: wgpu::PipelineLayout,
    /// The sun pass over the background: the frame only.
    sun_pipeline: wgpu::PipelineLayout,
    /// The star field (`stars.rs`): its parameters and band in group 1.
    pub stars: wgpu::BindGroupLayout,
    stars_pipeline: wgpu::PipelineLayout,
    stars_shader: wgpu::ShaderModule,
    /// The sun's disc and halo at their radiance into a bloom chain's glow
    /// (`fs_sun_glow`, `post.rs` `Chain::glow`): under no clouds, the flat
    /// layer, and the raymarched clouds, by `SkyClouds::index`.
    pub sun_glow: [wgpu::RenderPipeline; 3],
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
    /// Restores a shadow tile's static casters' depth from the static cache
    /// (`shadow_restore.wgsl`), and its cache binding.
    pub shadow_restore: wgpu::RenderPipeline,
    pub shadow_restore_layout: wgpu::BindGroupLayout,
    pub prepass: PrepassPipelines,
    /// Group 2 of the world pipelines: a view's ambient occlusion.
    pub ambient_occlusion: wgpu::BindGroupLayout,
    /// Product shaders that did not compose since last taken; their
    /// materials draw with the standard shade stage.
    pub shader_errors: Vec<String>,
}

/// One target format's pipelines: the sky, the sun and moon added over the
/// background, the cloud layer blended over both, and a world pipeline per
/// feature set and pass, created as materials need them.
pub(crate) struct Pipelines {
    pub target: ColorTarget,
    pub sky: wgpu::RenderPipeline,
    pub sun: wgpu::RenderPipeline,
    /// The moon added over the background after the sun (`fs_moon`).
    pub moon: wgpu::RenderPipeline,
    /// The star field's band and stars added over the panorama
    /// (`stars.wgsl`).
    pub star_band: wgpu::RenderPipeline,
    pub stars: wgpu::RenderPipeline,
    pub clouds: wgpu::RenderPipeline,
    /// The raymarched clouds' reduced target over the background
    /// (`fs_clouds_composite`).
    pub clouds_composite: wgpu::RenderPipeline,
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

/// The ambient occlusion pre-pass: the caster shaders drawing a view's
/// opaque batches depth-only with the world pass's culling and winding.
#[derive(Default)]
pub(crate) struct PrepassPipelines {
    pipelines: HashMap<(Features, Pass), wgpu::RenderPipeline>,
}

impl PrepassPipelines {
    /// A pipeline `Layouts::prepare_prepass` made before the pass was
    /// encoded.
    pub fn get(&self, pass: Pass, features: Features) -> &wgpu::RenderPipeline {
        &self.pipelines[&(features.caster(), pass)]
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

fn probe_texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D3,
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
        // The frame is visible to compute passes too, which light from it
        // (`volumetric_fog.rs`).
        let frame_entries = [
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
            storage_entry(7),
            // The sky's light (`sky_light.rs`): its cube, sampler and
            // irradiance harmonics.
            wgpu::BindGroupLayoutEntry {
                binding: 8,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    multisampled: false,
                },
                count: None,
            },
            sampler_entry(9),
            storage_entry(10),
            // The indirect light volume (`probes.rs`): one RGBA16F 3D
            // texture of L1 coefficients, the colour channels stacked
            // along its depth, and its sampler.
            probe_texture_entry(11),
            sampler_entry(12),
            // The cloud regions (`clouds.wgsl`).
            storage_entry(13),
        ]
        .map(|mut entry| {
            entry.visibility |= wgpu::ShaderStages::COMPUTE;
            entry
        });
        let frame = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu frame"),
            entries: &frame_entries,
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
        let ambient_occlusion = crate::ambient_occlusion::apply_layout(device);
        let world = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu world"),
            bind_group_layouts: &[Some(&frame), Some(&material), Some(&ambient_occlusion)],
            immediate_size: 0,
        });
        let water_depth = device.create_bind_group_layout(&crate::water::layout_descriptor());
        let water = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu water"),
            bind_group_layouts: &[Some(&frame), Some(&material), Some(&water_depth)],
            immediate_size: 0,
        });
        let sky_pipeline = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu sky"),
            bind_group_layouts: &[Some(&frame), Some(&sky)],
            immediate_size: 0,
        });
        let sun_pipeline = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu sun"),
            bind_group_layouts: &[Some(&frame)],
            immediate_size: 0,
        });
        let stars = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu stars"),
            entries: &[uniform_entry(0), texture_entry(1), sampler_entry(2)],
        });
        let stars_pipeline = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu stars"),
            bind_group_layouts: &[Some(&frame), Some(&stars)],
            immediate_size: 0,
        });
        let fragment_entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let filterable_2d = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let filtering = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let cloud_march = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu cloud march"),
            entries: &[
                fragment_entry(
                    0,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                fragment_entry(1, filterable_2d),
                fragment_entry(2, filtering),
            ],
        });
        let cloud_composite = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu cloud composite"),
            entries: &[
                fragment_entry(2, filtering),
                fragment_entry(3, filterable_2d),
            ],
        });
        let cloud_march_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu cloud march"),
            bind_group_layouts: &[Some(&frame), Some(&sky), Some(&cloud_march)],
            immediate_size: 0,
        });
        let cloud_composite_pipeline =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu cloud composite"),
                bind_group_layouts: &[Some(&frame), Some(&sky), Some(&cloud_composite)],
                immediate_size: 0,
            });
        let shadow_pipeline = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu shadow"),
            bind_group_layouts: &[Some(&casters), Some(&material), Some(&shadow_layer)],
            immediate_size: 0,
        });
        let mut shaders = Shaders::new();
        let sky_shader = standard(shaders.module(device, Entry::Sky, Features::default()));
        let stars_shader = standard(shaders.module(device, Entry::Stars, Features::default()));
        let compose = standard(shaders.module(device, Entry::Compose, Features::default()));
        // The reduced clouds: premultiplied colour and opacity, replaced.
        let cloud_march_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu cloud march"),
            layout: Some(&cloud_march_layout),
            vertex: wgpu::VertexState {
                module: &sky_shader,
                entry_point: Some("vs_sky"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &sky_shader,
                entry_point: Some("fs_clouds_march"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: crate::cloud_march::FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
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
        let shadow_restore_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render-wgpu shadow restore"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                }],
            });
        let restore = standard(shaders.module(device, Entry::ShadowRestore, Features::default()));
        let shadow_restore = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu shadow restore"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("render-wgpu shadow restore"),
                    bind_group_layouts: &[Some(&shadow_restore_layout)],
                    immediate_size: 0,
                }),
            ),
            vertex: wgpu::VertexState {
                module: &restore,
                entry_point: Some("vs_restore"),
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
            fragment: Some(wgpu::FragmentState {
                module: &restore,
                entry_point: Some("fs_restore"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        });
        // The glow replaces the chain's (cleared) glow target.
        let sun_glow = [
            (&sun_pipeline, "fs_sun_glow"),
            (&sky_pipeline, "fs_sun_glow_flat"),
            (&cloud_composite_pipeline, "fs_sun_glow_marched"),
        ]
        .map(|(layout, fragment)| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render-wgpu sun glow"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &sky_shader,
                    entry_point: Some("vs_sky"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &sky_shader,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(crate::post::BLOOM_FORMAT.into())],
                }),
                multiview_mask: None,
                cache: None,
            })
        });
        Self {
            frame,
            material,
            water,
            sky,
            casters,
            shadow_layer,
            world,
            sky_pipeline,
            cloud_march,
            cloud_march_pipeline,
            cloud_composite,
            cloud_composite_pipeline,
            sun_pipeline,
            stars,
            stars_pipeline,
            stars_shader,
            sun_glow,
            shadow_pipeline,
            shaders,
            sky_shader,
            world_shaders: HashMap::new(),
            shadow_shaders: HashMap::new(),
            shadow: ShadowPipelines::default(),
            shadow_clear,
            shadow_restore,
            shadow_restore_layout,
            prepass: PrepassPipelines::default(),
            ambient_occlusion,
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
        let background = |label, layout, fragment, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &self.sky_shader,
                    entry_point: Some("vs_sky"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                // The background pass draws without depth.
                depth_stencil: None,
                multisample: target.multisample(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.sky_shader,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target.format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let added = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        // Added over the background, as the sun is.
        let star_pipeline =
            |label, vertex, fragment, buffers: &[Option<wgpu::VertexBufferLayout<'_>>]| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(&self.stars_pipeline),
                    vertex: wgpu::VertexState {
                        module: &self.stars_shader,
                        entry_point: Some(vertex),
                        compilation_options: Default::default(),
                        buffers,
                    },
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: target.multisample(),
                    fragment: Some(wgpu::FragmentState {
                        module: &self.stars_shader,
                        entry_point: Some(fragment),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: target.format,
                            blend: Some(wgpu::BlendState {
                                color: added,
                                alpha: added,
                            }),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
            };
        Pipelines {
            target,
            sky: background("render-wgpu sky", &self.sky_pipeline, "fs_sky", None),
            sun: background(
                "render-wgpu sun",
                &self.sun_pipeline,
                "fs_sun",
                Some(wgpu::BlendState {
                    color: added,
                    alpha: added,
                }),
            ),
            moon: background(
                "render-wgpu moon",
                &self.sun_pipeline,
                "fs_moon",
                Some(wgpu::BlendState {
                    color: added,
                    alpha: added,
                }),
            ),
            star_band: star_pipeline("render-wgpu star band", "vs_star_band", "fs_star_band", &[]),
            stars: star_pipeline(
                "render-wgpu stars",
                "vs_star",
                "fs_star",
                &[Some(wgpu::VertexBufferLayout {
                    array_stride: crate::stars::STAR_BYTES,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3,
                        1 => Float32,
                        2 => Float32x3,
                    ],
                })],
            ),
            // Premultiplied over the background, which keeps its alpha.
            clouds: background(
                "render-wgpu clouds",
                &self.sky_pipeline,
                "fs_clouds",
                Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
            ),
            // The raymarched clouds' reduced target, blended as the flat
            // layer.
            clouds_composite: background(
                "render-wgpu volumetric clouds",
                &self.cloud_composite_pipeline,
                "fs_clouds_composite",
                Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
            ),
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
        let buffers = mesh_buffers(features, true);
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu world"),
            layout: Some(if features.contains(Features::WATER) {
                &self.water
            } else {
                &self.world
            }),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_world"),
                compilation_options: Default::default(),
                buffers: &buffers,
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
                // Opaque passes cover their pixel (`finish.rs`).
                entry_point: Some(if blend { "fs_world" } else { "fs_world_opaque" }),
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
        let buffers = mesh_buffers(features, false);
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu shadow"),
            layout: Some(&self.shadow_pipeline),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &buffers,
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
            fragment: features.caster_fragment().then(|| wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_shadow"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.shadow
            .pipelines
            .insert((features, cull_mode), pipeline);
        true
    }

    /// Make the ambient occlusion pre-pass pipeline drawing `features` in
    /// `pass` (an opaque world pass): the caster shader with the world pass's
    /// culling. Returns whether it was made now.
    pub fn prepare_prepass(
        &mut self,
        device: &wgpu::Device,
        features: Features,
        pass: Pass,
    ) -> bool {
        let features = features.caster();
        if self.prepass.pipelines.contains_key(&(features, pass)) {
            return false;
        }
        let shader = self
            .shadow_shaders
            .entry(features)
            .or_insert_with(|| standard(self.shaders.module(device, Entry::Shadow, features)));
        let (cull_mode, front_face) = match pass {
            Pass::OpaqueMirrored | Pass::BlendMirrored => (BACK, CW),
            Pass::OpaqueDoubleSided | Pass::BlendDoubleSided | Pass::Lines => (None, CCW),
            Pass::Opaque | Pass::Blend => (BACK, CCW),
        };
        let buffers = mesh_buffers(features, false);
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu ambient occlusion pre-pass"),
            layout: Some(&self.shadow_pipeline),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &buffers,
            },
            primitive: wgpu::PrimitiveState {
                cull_mode,
                front_face,
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
            fragment: features.caster_fragment().then(|| wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_shadow"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.prepass.pipelines.insert((features, pass), pipeline);
        true
    }
}

const TRIANGLES: wgpu::PrimitiveTopology = wgpu::PrimitiveTopology::TriangleList;
const BACK: Option<wgpu::Face> = Some(wgpu::Face::Back);
const CCW: wgpu::FrontFace = wgpu::FrontFace::Ccw;
const CW: wgpu::FrontFace = wgpu::FrontFace::Cw;
