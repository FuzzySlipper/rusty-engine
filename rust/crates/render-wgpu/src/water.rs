//! The water surfaces' view of the scene (`Features::WATER`): the opaque
//! pass's depth copied into a single-sample depth texture before the blend
//! pass, bound as group 2 of the water pipelines (`Layouts::water`, in place
//! of the view's occlusion, which blended parts never read), so a water
//! material reads what lies behind its surface
//! (`world.wgsl` `water_surface`). A view copies only when a water material
//! is among its blended parts; other views draw as before.
//!
//! Where the device binds one more texture than the world pass
//! (`Gpu::scene_color_refusal`), the copy can take the opaque pass's colour
//! too (`SCENE_COLOR`): a view copies it when one of its water materials
//! reads the scene (`Features::WATER_SCENE`: absorption, refraction) or it
//! reflects in its water (`RendererOptions::water_reflections`). Otherwise
//! group 2 binds a 1×1 stand-in that nothing reads.

use std::collections::HashMap;

use render_shaders::{Entry, Features};

use crate::finish::HDR_FORMAT;
use crate::gpu::Gpu;
use crate::pipelines::standard;
use crate::shaders::Shaders;
use crate::target::{TargetView, DEPTH_FORMAT};
use crate::timing::{GpuPassTiming, PassTimer};

/// The copy's pass in the GPU readout.
const PASS: &str = "water-copy";
/// The sampled textures a water pass binds with the scene's colour: the
/// material's 12, the frame's 3, the depth copy (16, WebGPU's guarantee),
/// then the colour copy.
pub(crate) const SCENE_SAMPLED_TEXTURES: u32 = 17;

/// The copy, its bind groups for the water pipelines and the copy pipelines
/// per sample count of the source and whether they copy the colour.
pub(crate) struct WaterScene {
    shader: wgpu::ShaderModule,
    /// Group 2 of the water pipelines: the depth copy, and with
    /// `scene_color` the colour copy and its sampler.
    layout: wgpu::BindGroupLayout,
    scene_color: bool,
    sampler: wgpu::Sampler,
    /// Group 2's colour where a view copies none.
    stand_in: wgpu::TextureView,
    /// Per source sample count and colour: the source's layout and the copy
    /// pipeline.
    copies: HashMap<(u32, bool), (wgpu::BindGroupLayout, wgpu::RenderPipeline)>,
    /// The copy for the last target size, and its bind groups.
    copy: Option<Copy>,
    /// Whether the last capture took the colour.
    colored: bool,
    timer: Option<PassTimer>,
    /// The last capture was timed and waits for its submit.
    timed: bool,
}

struct Copy {
    size: (u32, u32),
    depth: wgpu::TextureView,
    /// Made the first time a view copies the colour at this size.
    color: Option<wgpu::TextureView>,
    /// The depth copy with the stand-in colour.
    plain: wgpu::BindGroup,
    /// The depth and colour copies.
    scene: Option<wgpu::BindGroup>,
}

impl WaterScene {
    pub fn new(gpu: &Gpu, shaders: &mut Shaders, scene_color: bool) -> Self {
        let device = &gpu.device;
        let shader = standard(shaders.module(device, Entry::WaterDepth, Features::default()));
        let stand_in = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("render-wgpu water scene stand-in"),
                size: crate::target::extent(1, 1),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Self {
            shader,
            // The same layout `Layouts` gives the water pipelines' group 2.
            layout: device.create_bind_group_layout(&layout_descriptor(scene_color)),
            scene_color,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu water scene"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            stand_in,
            copies: HashMap::new(),
            copy: None,
            colored: false,
            timer: PassTimer::new(gpu, PASS),
            timed: false,
        }
    }

    /// Whether a blended draw list holds a water material, and whether one
    /// of them reads the scene's colour.
    pub fn wanted(features: impl Iterator<Item = Features>) -> (bool, bool) {
        features.fold((false, false), |(water, scene), features| {
            (
                water || features.contains(Features::WATER),
                scene || features.contains(Features::WATER_SCENE),
            )
        })
    }

    /// Whether water may take the colour copy on this device.
    pub fn scene_color(&self) -> bool {
        self.scene_color
    }

    /// Before the frame's views: take the last timed copy's time.
    pub fn begin_frame(&mut self, gpu: &Gpu) {
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
    }

    /// Copy `target`'s depth (the opaque pass done) for the water surfaces
    /// the blend pass draws next, and with `color` the opaque colour too
    /// (single-sample or multisampled, as the target), where the device
    /// binds it.
    pub fn capture(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &TargetView<'_>,
        color: Option<&wgpu::TextureView>,
    ) {
        let color = color.filter(|_| self.scene_color);
        let size = (target.width, target.height);
        if self.copy.as_ref().is_none_or(|copy| copy.size != size) {
            let depth = gpu
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("render-wgpu water depth"),
                    size: crate::target::extent(size.0, size.1),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: DEPTH_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            let plain = self.make_bind_group(gpu, &depth, &self.stand_in);
            self.copy = Some(Copy {
                size,
                depth,
                color: None,
                plain,
                scene: None,
            });
        }
        if color.is_some() && self.copy.as_ref().is_some_and(|copy| copy.color.is_none()) {
            let view = gpu
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("render-wgpu water scene colour"),
                    size: crate::target::extent(size.0, size.1),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: HDR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            let copy = self.copy.as_ref().expect("made above");
            let scene = self.make_bind_group(gpu, &copy.depth, &view);
            let copy = self.copy.as_mut().expect("made above");
            copy.color = Some(view);
            copy.scene = Some(scene);
        }
        let samples = target.samples;
        let colored = color.is_some();
        let shader = &self.shader;
        let (source_layout, pipeline) = self
            .copies
            .entry((samples, colored))
            .or_insert_with(|| copy_pipeline(&gpu.device, shader, samples, colored));
        let multisampled = u32::from(samples > 1);
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: multisampled,
            resource: wgpu::BindingResource::TextureView(target.depth),
        }];
        if let Some(color) = color {
            entries.push(wgpu::BindGroupEntry {
                binding: 2 + multisampled,
                resource: wgpu::BindingResource::TextureView(color),
            });
        }
        let source = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu water scene source"),
            layout: source_layout,
            entries: &entries,
        });
        let copy = self.copy.as_ref().expect("made above");
        let color_attachment =
            copy.color
                .as_ref()
                .filter(|_| colored)
                .map(|view| wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                });
        let timed = self.timer.as_ref().and_then(PassTimer::render_writes);
        let is_timed = timed.is_some();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render-wgpu water scene copy"),
            color_attachments: if color_attachment.is_some() {
                std::slice::from_ref(&color_attachment)
            } else {
                &[]
            },
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &copy.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: timed,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &source, &[]);
        pass.draw(0..3, 0..1);
        drop(pass);
        self.timed = is_timed;
        if colored != self.colored {
            // The copy's cost changes with what it takes.
            if let Some(timer) = &mut self.timer {
                timer.restart();
            }
            self.colored = colored;
        }
        if let Some(timer) = self.timer.as_mut().filter(|_| is_timed) {
            timer.resolve(encoder);
        }
    }

    /// After the encoder holding a capture was submitted.
    pub fn submitted(&mut self) {
        if let Some(timer) = self.timer.as_mut().filter(|_| self.timed) {
            timer.submitted();
        }
        self.timed = false;
    }

    /// Group 2 of a water pipeline, after `capture`: with the colour copy
    /// when this view's capture took it.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        let copy = self.copy.as_ref().expect("captured this view");
        match &copy.scene {
            Some(scene) if self.colored => scene,
            _ => &copy.plain,
        }
    }

    pub fn timing(&self) -> GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| crate::timing::untimed(PASS), PassTimer::readout)
    }

    fn make_bind_group(
        &self,
        gpu: &Gpu,
        depth: &wgpu::TextureView,
        color: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(depth),
        }];
        if self.scene_color {
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(color),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            });
        }
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu water scene"),
            layout: &self.layout,
            entries: &entries,
        })
    }
}

/// The copy pass for a source of `samples` per pixel: its depth, and with
/// `colored` its colour (multisampled colour averaged).
fn copy_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    samples: u32,
    colored: bool,
) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline) {
    let multisampled = samples > 1;
    let mut entries = vec![wgpu::BindGroupLayoutEntry {
        binding: u32::from(multisampled),
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled,
        },
        count: None,
    }];
    if colored {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 2 + u32::from(multisampled),
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float {
                    filterable: !multisampled,
                },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled,
            },
            count: None,
        });
    }
    let source_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu water scene source"),
        entries: &entries,
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("render-wgpu water scene copy"),
        bind_group_layouts: &[Some(&source_layout)],
        immediate_size: 0,
    });
    let entry_point = match (colored, multisampled) {
        (false, false) => "fs_copy_depth",
        (false, true) => "fs_copy_depth_multisampled",
        (true, false) => "fs_copy_scene",
        (true, true) => "fs_copy_scene_multisampled",
    };
    let color_target = [Some(wgpu::ColorTargetState {
        format: HDR_FORMAT,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    })];
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("render-wgpu water scene copy"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
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
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            targets: if colored { &color_target } else { &[] },
        }),
        multiview_mask: None,
        cache: None,
    });
    (source_layout, pipeline)
}

/// Group 2 of the water pipelines: the single-sample depth copy, and with
/// `scene_color` the colour copy and its sampler.
pub(crate) fn layout_descriptor(scene_color: bool) -> wgpu::BindGroupLayoutDescriptor<'static> {
    const SCENE: [wgpu::BindGroupLayoutEntry; 3] = [
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ];
    wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu water scene"),
        entries: if scene_color { &SCENE } else { &SCENE[..1] },
    }
}
