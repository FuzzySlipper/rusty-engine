//! Screen-space ambient occlusion for a world view pass
//! (`ambient_occlusion.wgsl`), the first compute candidate (#9510) on the
//! compute pass foundation (#9509).
//!
//! Before a world view pass draws, its opaque batches draw depth only into a
//! half-resolution depth texture (the pre-pass, encoded by `frame.rs` with
//! the shadow caster shaders and the camera in the shadow matrices' camera
//! slot). The occlusion pass reads that depth, by the compute path (a
//! workgroup's depth tile in shared memory) or the raster path (a full-screen
//! triangle), into a half-resolution texture; a separable depth-aware blur
//! smooths it;
//! and the world pass samples the result by target pixel into
//! `Surface.occlusion`, which scales the ambient and hemisphere light.
//!
//! The host chooses the path and the product its strength
//! (`RendererOptions::ambient_occlusion`). An adapter without compute shaders
//! or with workgroup limits below the kernel's takes the raster path and the
//! readout says why. Each pass is timed through `timing.rs` so the two paths
//! compare on any adapter.

use crate::camera::CameraMatrices;
use crate::frame::PixelRect;
use crate::target::{extent, DEPTH_FORMAT};
use crate::timing::{untimed, GpuPassTiming, PassTimer};
use crate::Gpu;

/// The occlusion textures: filterable and a storage texture, so both paths
/// write it and the world pass samples it bilinearly.
const OCCLUSION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// `@workgroup_size` of `cs_occlusion`.
const WORKGROUP: u32 = 16;
/// `TILE_APRON` in the shader: the largest sample radius in texels.
const TILE_APRON: f32 = 8.0;
/// Shared memory `cs_occlusion` needs: the 32×32 depth tile.
const TILE_BYTES: u32 = (WORKGROUP + 2 * TILE_APRON as u32).pow(2) * 4;
/// How far a surface darkens its neighbours, in world units.
const RADIUS: f32 = 0.75;
/// Cosine below which a sample does not occlude: keeps flat surfaces clean.
const BIAS: f32 = 0.05;
/// Scales the summed occlusion before the strength.
const INTENSITY: f32 = 3.0;
/// `AoParams`: two matrices, the region and the tuning.
const PARAMS_BYTES: u64 = 64 + 64 + 16 + 16;
/// `AmbientOcclusion` in world.wgsl.
const APPLY_BYTES: u64 = 16;
/// Target sizes kept ready at once (the primary and a few offscreen sizes).
const MAX_TARGETS: usize = 4;

const PREPASS: &str = "ao-prepass";
const OCCLUSION: &str = "ao-occlusion";
const BLUR: &str = "ao-blur";

/// Which pass computes the occlusion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbientOcclusionPath {
    Off,
    /// `cs_occlusion`: a shared-memory depth tile per workgroup.
    Compute,
    /// `fs_occlusion`: a full-screen triangle sampling the depth texture.
    Raster,
}

/// The host's ambient occlusion choice: the path, and the product's
/// strength (0 draws without it, 1 the full occlusion).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientOcclusion {
    pub path: AmbientOcclusionPath,
    pub strength: f32,
}

impl Default for AmbientOcclusion {
    fn default() -> Self {
        Self {
            path: AmbientOcclusionPath::Off,
            strength: 1.0,
        }
    }
}

/// The adapter's compute limits. The device takes wgpu's default limits
/// (`gpu.rs`); a kernel needing more raises them there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputeLimits {
    pub workgroup_size: [u32; 3],
    pub invocations_per_workgroup: u32,
    pub workgroups_per_dimension: u32,
    pub workgroup_storage_bytes: u32,
    pub storage_buffer_binding_bytes: u64,
}

impl ComputeLimits {
    fn of(limits: &wgpu::Limits) -> Self {
        Self {
            workgroup_size: [
                limits.max_compute_workgroup_size_x,
                limits.max_compute_workgroup_size_y,
                limits.max_compute_workgroup_size_z,
            ],
            invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
            workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
            workgroup_storage_bytes: limits.max_compute_workgroup_storage_size,
            storage_buffer_binding_bytes: limits.max_storage_buffer_binding_size,
        }
    }
}

/// What the renderer's GPU passes report, for diagnostics and evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuReadout {
    /// Why the compute path cannot run on this adapter; `None` while it can.
    /// The raster path runs either way.
    pub compute_refused: Option<String>,
    /// The device has timestamp queries, so the passes are timed.
    pub timestamps: bool,
    pub limits: ComputeLimits,
    /// The path the last world view's occlusion took (`Raster` when compute
    /// was asked for but refused).
    pub ambient_occlusion: AmbientOcclusionPath,
    /// Workgroups the last occlusion dispatch took; 0 on the raster path.
    pub workgroups: u32,
    /// The occlusion texture of the last view: half its target.
    pub occlusion_texture: (u32, u32),
    /// The timed passes: pre-pass, occlusion, blur.
    pub passes: Vec<GpuPassTiming>,
}

/// One world view's occlusion this frame: its target's resources and the
/// half-resolution region its viewport covers.
pub(crate) struct ViewOcclusion {
    target: usize,
    region: PixelRect,
    path: AmbientOcclusionPath,
}

struct Compute {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
}

/// The textures and bind groups for one target size.
struct Target {
    size: (u32, u32),
    texture: (u32, u32),
    depth: wgpu::TextureView,
    raw: wgpu::TextureView,
    /// Blurred along x, the vertical pass's input.
    half_blurred: wgpu::TextureView,
    blurred: wgpu::TextureView,
    params: wgpu::Buffer,
    apply_params: wgpu::Buffer,
    compute: Option<wgpu::BindGroup>,
    raster: wgpu::BindGroup,
    blur_x: wgpu::BindGroup,
    blur_y: wgpu::BindGroup,
    apply: wgpu::BindGroup,
}

pub(crate) struct AmbientOcclusionPass {
    compute: Result<Compute, String>,
    raster_layout: wgpu::BindGroupLayout,
    raster: wgpu::RenderPipeline,
    blur_layout: wgpu::BindGroupLayout,
    blur_x: wgpu::RenderPipeline,
    blur_y: wgpu::RenderPipeline,
    apply_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Group 2 of a world pass without occlusion: white, strength 0.
    off: wgpu::BindGroup,
    targets: Vec<Target>,
    prepass_timer: Option<PassTimer>,
    occlusion_timer: Option<PassTimer>,
    blur_timer: Option<PassTimer>,
    limits: ComputeLimits,
    last_path: AmbientOcclusionPath,
    last_workgroups: u32,
    last_texture: (u32, u32),
}

/// Group 2 of the world pipelines: the occlusion map, its sampler and the
/// strength.
pub(crate) fn apply_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu ambient occlusion apply"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

fn params_entry(visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn depth_entry(visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 1,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// Why this adapter cannot run `cs_occlusion`, if it cannot.
fn compute_refusal(gpu: &Gpu) -> Result<(), String> {
    let downlevel = gpu.adapter.get_downlevel_capabilities();
    if !downlevel
        .flags
        .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
    {
        return Err("the adapter has no compute shaders".to_owned());
    }
    let granted = ComputeLimits::of(&gpu.device.limits());
    if granted.workgroup_size[0] < WORKGROUP
        || granted.workgroup_size[1] < WORKGROUP
        || granted.invocations_per_workgroup < WORKGROUP * WORKGROUP
    {
        return Err(format!(
            "the device allows {} invocations per workgroup ({}×{} along x and y); the occlusion kernel needs {WORKGROUP}×{WORKGROUP}",
            granted.invocations_per_workgroup, granted.workgroup_size[0], granted.workgroup_size[1]
        ));
    }
    if granted.workgroup_storage_bytes < TILE_BYTES {
        return Err(format!(
            "the device allows {} bytes of workgroup storage; the occlusion kernel's depth tile needs {TILE_BYTES}",
            granted.workgroup_storage_bytes
        ));
    }
    Ok(())
}

impl AmbientOcclusionPass {
    pub fn new(
        gpu: &Gpu,
        shader: wgpu::ShaderModule,
        apply_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let device = &gpu.device;
        let compute = compute_refusal(gpu).map(|()| {
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render-wgpu ambient occlusion compute"),
                entries: &[
                    params_entry(wgpu::ShaderStages::COMPUTE),
                    depth_entry(wgpu::ShaderStages::COMPUTE),
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: OCCLUSION_FORMAT,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu ambient occlusion compute"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("render-wgpu ambient occlusion compute"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("cs_occlusion"),
                compilation_options: Default::default(),
                cache: None,
            });
            Compute { layout, pipeline }
        });
        let raster_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu ambient occlusion raster"),
            entries: &[
                params_entry(wgpu::ShaderStages::FRAGMENT),
                depth_entry(wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu ambient occlusion blur"),
            entries: &[
                params_entry(wgpu::ShaderStages::FRAGMENT),
                depth_entry(wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let fullscreen = |label: &str,
                          layout: &wgpu::BindGroupLayout,
                          fragment: &str,
                          constants: &[(&str, f64)]| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_fullscreen"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants,
                        ..Default::default()
                    },
                    targets: &[Some(OCCLUSION_FORMAT.into())],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let raster = fullscreen(
            "render-wgpu ambient occlusion raster",
            &raster_layout,
            "fs_occlusion",
            &[],
        );
        let blur_x = fullscreen(
            "render-wgpu ambient occlusion blur x",
            &blur_layout,
            "fs_blur",
            &[("BLUR_AXIS", 0.0)],
        );
        let blur_y = fullscreen(
            "render-wgpu ambient occlusion blur y",
            &blur_layout,
            "fs_blur",
            &[("BLUR_AXIS", 1.0)],
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("render-wgpu ambient occlusion"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        use wgpu::util::DeviceExt;
        let white = device
            .create_texture_with_data(
                &gpu.queue,
                &wgpu::TextureDescriptor {
                    label: Some("render-wgpu ambient occlusion off"),
                    size: extent(1, 1),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: OCCLUSION_FORMAT,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &[255; 4],
            )
            .create_view(&Default::default());
        let off_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("render-wgpu ambient occlusion off"),
            contents: &[0; APPLY_BYTES as usize],
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let off = apply_bind_group(device, apply_layout, &white, &sampler, &off_params);
        Self {
            compute,
            raster_layout,
            raster,
            blur_layout,
            blur_x,
            blur_y,
            apply_layout: apply_layout.clone(),
            sampler,
            off,
            targets: Vec::new(),
            prepass_timer: PassTimer::new(gpu, PREPASS),
            occlusion_timer: PassTimer::new(gpu, OCCLUSION),
            blur_timer: PassTimer::new(gpu, BLUR),
            limits: ComputeLimits::of(&gpu.adapter.limits()),
            last_path: AmbientOcclusionPath::Off,
            last_workgroups: 0,
            last_texture: (0, 0),
        }
    }

    /// Ready a world view's occlusion: its target's textures, this view's
    /// parameters, and the path it takes. `None` when occlusion is off.
    pub fn begin_view(
        &mut self,
        gpu: &Gpu,
        options: AmbientOcclusion,
        size: (u32, u32),
        area: PixelRect,
        camera: &CameraMatrices,
    ) -> Option<ViewOcclusion> {
        let path = match options.path {
            AmbientOcclusionPath::Off => return None,
            _ if options.strength <= 0.0 || options.strength.is_nan() => return None,
            AmbientOcclusionPath::Compute if self.compute.is_ok() => AmbientOcclusionPath::Compute,
            AmbientOcclusionPath::Compute | AmbientOcclusionPath::Raster => {
                AmbientOcclusionPath::Raster
            }
        };
        for timer in [
            &mut self.prepass_timer,
            &mut self.occlusion_timer,
            &mut self.blur_timer,
        ]
        .into_iter()
        .flatten()
        {
            timer.collect(gpu);
        }
        let target = self.target_index(gpu, size);
        let region = PixelRect {
            x: area.x / 2,
            y: area.y / 2,
            width: (area.x + area.width).div_ceil(2) - area.x / 2,
            height: (area.y + area.height).div_ceil(2) - area.y / 2,
        };
        let mut params = Vec::with_capacity(PARAMS_BYTES as usize);
        let inverse = camera.projection.inverse();
        params.extend_from_slice(bytemuck::cast_slice(&inverse.to_cols_array()));
        params.extend_from_slice(bytemuck::cast_slice(&camera.projection.to_cols_array()));
        for value in [region.x, region.y, region.width, region.height] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        for value in [RADIUS, BIAS, INTENSITY, TILE_APRON] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        let entry = &self.targets[target];
        gpu.queue.write_buffer(&entry.params, 0, &params);
        let mut apply = Vec::with_capacity(APPLY_BYTES as usize);
        for value in [
            options.strength.min(1.0),
            1.0 / size.0 as f32,
            1.0 / size.1 as f32,
            0.0,
        ] {
            apply.extend_from_slice(&value.to_le_bytes());
        }
        gpu.queue.write_buffer(&entry.apply_params, 0, &apply);
        self.last_path = path;
        self.last_texture = entry.texture;
        if path != AmbientOcclusionPath::Compute {
            self.last_workgroups = 0;
        }
        Some(ViewOcclusion {
            target,
            region,
            path,
        })
    }

    /// The pre-pass: `draw` encodes the view's opaque batches depth-only
    /// into the half-resolution depth, over the view's region. [`Self::encode`]
    /// follows it in the same encoder.
    pub fn encode_prepass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &ViewOcclusion,
        draw: impl FnOnce(&mut wgpu::RenderPass<'_>),
    ) {
        let target = &self.targets[view.target];
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(PREPASS),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self
                    .prepass_timer
                    .as_ref()
                    .and_then(PassTimer::render_writes),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            set_region(&mut pass, view.region);
            draw(&mut pass);
        }
    }

    /// The occlusion pass by the view's path, then the blur.
    pub fn encode(&mut self, encoder: &mut wgpu::CommandEncoder, view: &ViewOcclusion) {
        if let Some(timer) = &mut self.prepass_timer {
            timer.resolve(encoder);
        }
        let target = &self.targets[view.target];
        match (view.path, &self.compute, &target.compute) {
            (AmbientOcclusionPath::Compute, Ok(compute), Some(bind_group)) => {
                let workgroups = (
                    view.region.width.div_ceil(WORKGROUP),
                    view.region.height.div_ceil(WORKGROUP),
                );
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(OCCLUSION),
                    timestamp_writes: self
                        .occlusion_timer
                        .as_ref()
                        .and_then(PassTimer::compute_writes),
                });
                pass.set_pipeline(&compute.pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(workgroups.0, workgroups.1, 1);
                drop(pass);
                self.last_workgroups = workgroups.0 * workgroups.1;
            }
            _ => {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(OCCLUSION),
                    color_attachments: &[Some(occlusion_attachment(&target.raw))],
                    depth_stencil_attachment: None,
                    timestamp_writes: self
                        .occlusion_timer
                        .as_ref()
                        .and_then(PassTimer::render_writes),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                set_region(&mut pass, view.region);
                pass.set_pipeline(&self.raster);
                pass.set_bind_group(0, &target.raster, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        if let Some(timer) = &mut self.occlusion_timer {
            timer.resolve(encoder);
        }
        // The blur's two passes share the blur timer's begin and end stamps.
        for (index, (pipeline, bind_group, output)) in [
            (&self.blur_x, &target.blur_x, &target.half_blurred),
            (&self.blur_y, &target.blur_y, &target.blurred),
        ]
        .into_iter()
        .enumerate()
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(BLUR),
                color_attachments: &[Some(occlusion_attachment(output))],
                depth_stencil_attachment: None,
                timestamp_writes: self
                    .blur_timer
                    .as_ref()
                    .and_then(|timer| timer.render_writes_between(index == 0, index == 1)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            set_region(&mut pass, view.region);
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(timer) = &mut self.blur_timer {
            timer.resolve(encoder);
        }
    }

    /// Group 2 of the world pass: the view's blurred occlusion, or the off
    /// group.
    pub fn apply_bind_group(&self, view: Option<&ViewOcclusion>) -> &wgpu::BindGroup {
        match view {
            Some(view) => &self.targets[view.target].apply,
            None => &self.off,
        }
    }

    /// After the view's encoder was submitted: read the timed passes back.
    pub fn submitted(&mut self) {
        for timer in [
            &mut self.prepass_timer,
            &mut self.occlusion_timer,
            &mut self.blur_timer,
        ]
        .into_iter()
        .flatten()
        {
            timer.submitted();
        }
    }

    pub fn readout(&self) -> GpuReadout {
        let timing = |timer: &Option<PassTimer>, pass| {
            timer
                .as_ref()
                .map_or_else(|| untimed(pass), PassTimer::readout)
        };
        GpuReadout {
            compute_refused: self.compute.as_ref().err().cloned(),
            timestamps: self.occlusion_timer.is_some(),
            limits: self.limits,
            ambient_occlusion: self.last_path,
            workgroups: self.last_workgroups,
            occlusion_texture: self.last_texture,
            passes: vec![
                timing(&self.prepass_timer, PREPASS),
                timing(&self.occlusion_timer, OCCLUSION),
                timing(&self.blur_timer, BLUR),
            ],
        }
    }

    /// The resources for a target size, made on first use; the least
    /// recently made set goes when more than `MAX_TARGETS` are held.
    fn target_index(&mut self, gpu: &Gpu, size: (u32, u32)) -> usize {
        if let Some(index) = self.targets.iter().position(|target| target.size == size) {
            return index;
        }
        if self.targets.len() == MAX_TARGETS {
            self.targets.remove(0);
        }
        let device = &gpu.device;
        let texture = (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1));
        let make = |label: &str, format, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: extent(texture.0, texture.1),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let depth = make(
            "render-wgpu ambient occlusion depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let raw = make(
            "render-wgpu ambient occlusion",
            OCCLUSION_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let half_blurred = make(
            "render-wgpu ambient occlusion half blurred",
            OCCLUSION_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let blurred = make(
            "render-wgpu ambient occlusion blurred",
            OCCLUSION_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let uniform = |label: &str, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let params = uniform("render-wgpu ambient occlusion params", PARAMS_BYTES);
        let apply_params = uniform("render-wgpu ambient occlusion apply", APPLY_BYTES);
        let params_binding = wgpu::BindGroupEntry {
            binding: 0,
            resource: params.as_entire_binding(),
        };
        let depth_binding = wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::TextureView(&depth),
        };
        let compute = self.compute.as_ref().ok().map(|compute| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu ambient occlusion compute"),
                layout: &compute.layout,
                entries: &[
                    params_binding.clone(),
                    depth_binding.clone(),
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&raw),
                    },
                ],
            })
        });
        let raster = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu ambient occlusion raster"),
            layout: &self.raster_layout,
            entries: &[params_binding.clone(), depth_binding.clone()],
        });
        let blur = |label, input: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &self.blur_layout,
                entries: &[
                    params_binding.clone(),
                    depth_binding.clone(),
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(input),
                    },
                ],
            })
        };
        let blur_x = blur("render-wgpu ambient occlusion blur x", &raw);
        let blur_y = blur("render-wgpu ambient occlusion blur y", &half_blurred);
        let apply = apply_bind_group(
            device,
            &self.apply_layout,
            &blurred,
            &self.sampler,
            &apply_params,
        );
        self.targets.push(Target {
            size,
            texture,
            depth,
            raw,
            half_blurred,
            blurred,
            params,
            apply_params,
            compute,
            raster,
            blur_x,
            blur_y,
            apply,
        });
        self.targets.len() - 1
    }
}

fn apply_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    occlusion: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    params: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("render-wgpu ambient occlusion apply"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(occlusion),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params.as_entire_binding(),
            },
        ],
    })
}

fn occlusion_attachment(view: &wgpu::TextureView) -> wgpu::RenderPassColorAttachment<'_> {
    wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
            store: wgpu::StoreOp::Store,
        },
    }
}

fn set_region(pass: &mut wgpu::RenderPass<'_>, region: PixelRect) {
    pass.set_viewport(
        region.x as f32,
        region.y as f32,
        region.width as f32,
        region.height as f32,
        0.0,
        1.0,
    );
    pass.set_scissor_rect(region.x, region.y, region.width, region.height);
}
