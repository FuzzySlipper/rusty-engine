//! Occlusion of selected scatter copies behind a world view's opaque scene
//! (#10020, `hiz.wgsl`).
//!
//! With `RendererOptions::occlusion_culling`, a world view that selects
//! scatter copies (`scatter_select.rs`) first draws its opaque non-scatter
//! batches depth-only: the ambient occlusion pre-pass when it runs (its
//! depth then serves both), otherwise a half-resolution depth pass of its
//! own. That depth is reduced to a farthest-depth pyramid, and the
//! selection drops from the camera's list a copy whose bounds lie wholly
//! behind it. The depth is this frame's, so camera motion, rebases and
//! edits need nothing more, and nothing the selection keeps is missing for
//! a frame. A masked part writes the depth of what it keeps, so a canopy's
//! holes stay open; scatter copies never occlude. The shadows' list is not
//! tested.

use crate::frame::PixelRect;
use crate::target::DEPTH_FORMAT;
use crate::timing::{untimed, GpuPassTiming, PassTimer};
use crate::Gpu;

const PASS: &str = "occlusion";
const WORKGROUP: u32 = 8;
/// `Seed` in hiz.wgsl.
const SEED_BYTES: u64 = 16;
const PYRAMID_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

/// The occlusion test of the last world view, for diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OcclusionReadout {
    /// The last world view tested its selected copies against its depth.
    pub enabled: bool,
    /// Why this device cannot; `None` while it can.
    pub refused: Option<String>,
    /// The pyramid's level-0 size (the view's region, padded to powers of
    /// two) and its levels.
    pub size: [u32; 2],
    pub levels: u32,
    /// Its depth was the ambient occlusion pre-pass's, with no pass of its
    /// own.
    pub shared_depth: bool,
}

/// Where a view's occluder depth comes from.
pub(crate) enum OccluderDepth<'a> {
    /// The ambient occlusion pre-pass's depth, over this region of it.
    Shared(&'a wgpu::TextureView, PixelRect),
    /// [`Occlusion::encode_depth`]'s, drawn just before over this region.
    Own(PixelRect),
}

/// What the selection tests against: the pyramid and the size of the
/// view's region in its level-0 texels, and its levels.
pub(crate) struct Occluders<'a> {
    pub pyramid: &'a wgpu::TextureView,
    pub region: [u32; 2],
    pub levels: u32,
}

struct Pipelines {
    seed_layout: wgpu::BindGroupLayout,
    reduce_layout: wgpu::BindGroupLayout,
    seed: wgpu::ComputePipeline,
    reduce: wgpu::ComputePipeline,
}

struct Pyramid {
    size: (u32, u32),
    levels: u32,
    all: wgpu::TextureView,
    level_views: Vec<wgpu::TextureView>,
}

pub(crate) struct Occlusion {
    pipelines: Result<Pipelines, String>,
    seed: wgpu::Buffer,
    pyramid: Option<Pyramid>,
    /// The view's own half-resolution depth, when no pre-pass shares one.
    depth: Option<((u32, u32), wgpu::TextureView)>,
    region: [u32; 2],
    timer: Option<PassTimer>,
    readout: OcclusionReadout,
}

impl Occlusion {
    pub fn new(gpu: &Gpu, shader: wgpu::ShaderModule) -> Self {
        let device = &gpu.device;
        let pipelines = match gpu.compute_refusal([WORKGROUP, WORKGROUP, 1], 0) {
            Some(refused) => Err(refused),
            None => {
                let storage = |binding| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: PYRAMID_FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                };
                let seed_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("render-wgpu occlusion seed"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Depth,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            storage(2),
                        ],
                    });
                let reduce_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("render-wgpu occlusion reduce"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            storage(4),
                        ],
                    });
                let pipeline = |layout: &wgpu::BindGroupLayout, entry| {
                    let pipeline_layout =
                        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: Some("render-wgpu occlusion"),
                            bind_group_layouts: &[Some(layout)],
                            immediate_size: 0,
                        });
                    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("render-wgpu occlusion"),
                        layout: Some(&pipeline_layout),
                        module: &shader,
                        entry_point: Some(entry),
                        compilation_options: Default::default(),
                        cache: None,
                    })
                };
                Ok(Pipelines {
                    seed: pipeline(&seed_layout, "cs_seed"),
                    reduce: pipeline(&reduce_layout, "cs_reduce"),
                    seed_layout,
                    reduce_layout,
                })
            }
        };
        Self {
            seed: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu occlusion seed"),
                size: SEED_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            readout: OcclusionReadout {
                refused: pipelines.as_ref().err().cloned(),
                ..Default::default()
            },
            pipelines,
            pyramid: None,
            depth: None,
            region: [0, 0],
            timer: PassTimer::new(gpu, PASS),
        }
    }

    pub fn available(&self) -> bool {
        self.pipelines.is_ok()
    }

    /// Ready the view's own half-resolution occluder depth for a target of
    /// `size`, and the region `area` covers in it. Starts the occlusion's
    /// timing.
    pub fn prepare_depth(&mut self, gpu: &Gpu, size: (u32, u32), area: PixelRect) -> PixelRect {
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        let half = (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1));
        if self.depth.as_ref().is_none_or(|(at, _)| *at != half) {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("render-wgpu occlusion depth"),
                size: wgpu::Extent3d {
                    width: half.0,
                    height: half.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.depth = Some((half, texture.create_view(&Default::default())));
        }
        PixelRect {
            x: area.x / 2,
            y: area.y / 2,
            width: (area.x + area.width).div_ceil(2) - area.x / 2,
            height: (area.y + area.height).div_ceil(2) - area.y / 2,
        }
    }

    /// The view's own occluder depth pass over `region`
    /// ([`Self::prepare_depth`]): `draw` encodes the opaque non-scatter
    /// batches into it.
    pub fn encode_depth(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        region: PixelRect,
        draw: impl FnOnce(&mut wgpu::RenderPass<'_>),
    ) {
        let (_, depth) = self.depth.as_ref().expect("prepare_depth made it");
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("occlusion-depth"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: self
                .timer
                .as_ref()
                .and_then(|timer| timer.render_writes_between(true, false)),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(
            region.x as f32,
            region.y as f32,
            region.width as f32,
            region.height as f32,
            0.0,
            1.0,
        );
        pass.set_scissor_rect(region.x, region.y, region.width, region.height);
        draw(&mut pass);
    }

    /// Reduce the view's occluder depth to the pyramid the selection tests
    /// against. Ends the occlusion's timing (starts it too for shared
    /// depth).
    pub fn build(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        source: OccluderDepth<'_>,
    ) {
        let Ok(pipelines) = &self.pipelines else {
            return;
        };
        let shared = matches!(source, OccluderDepth::Shared(..));
        if shared {
            if let Some(timer) = &mut self.timer {
                timer.collect(gpu);
            }
        }
        let (depth, region) = match source {
            OccluderDepth::Shared(depth, region) => (depth.clone(), region),
            OccluderDepth::Own(region) => (
                self.depth
                    .as_ref()
                    .expect("prepare_depth made it")
                    .1
                    .clone(),
                region,
            ),
        };
        let size = (
            region.width.max(1).next_power_of_two(),
            region.height.max(1).next_power_of_two(),
        );
        if self
            .pyramid
            .as_ref()
            .is_none_or(|pyramid| pyramid.size != size)
        {
            let levels = size.0.max(size.1).ilog2() + 1;
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("render-wgpu occlusion pyramid"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: PYRAMID_FORMAT,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let level_views = (0..levels)
                .map(|level| {
                    texture.create_view(&wgpu::TextureViewDescriptor {
                        base_mip_level: level,
                        mip_level_count: Some(1),
                        ..Default::default()
                    })
                })
                .collect();
            self.pyramid = Some(Pyramid {
                size,
                levels,
                all: texture.create_view(&Default::default()),
                level_views,
            });
        }
        let pyramid = self.pyramid.as_ref().expect("made above");
        let mut seed = Vec::with_capacity(SEED_BYTES as usize);
        for value in [region.x, region.y, region.width, region.height] {
            seed.extend_from_slice(&value.to_le_bytes());
        }
        gpu.queue.write_buffer(&self.seed, 0, &seed);
        let groups = |level: u32| {
            (
                (size.0 >> level).max(1).div_ceil(WORKGROUP),
                (size.1 >> level).max(1).div_ceil(WORKGROUP),
            )
        };
        let seed_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu occlusion seed"),
            layout: &pipelines.seed_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.seed.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&pyramid.level_views[0]),
                },
            ],
        });
        let last = pyramid.levels - 1;
        for level in 0..pyramid.levels {
            let writes = self.timer.as_ref().and_then(|timer| {
                timer.compute_writes_between(shared && level == 0, level == last)
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(PASS),
                timestamp_writes: writes,
            });
            let (x, y) = groups(level);
            if level == 0 {
                pass.set_pipeline(&pipelines.seed);
                pass.set_bind_group(0, &seed_group, &[]);
            } else {
                let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("render-wgpu occlusion reduce"),
                    layout: &pipelines.reduce_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(
                                &pyramid.level_views[level as usize - 1],
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(
                                &pyramid.level_views[level as usize],
                            ),
                        },
                    ],
                });
                pass.set_pipeline(&pipelines.reduce);
                pass.set_bind_group(0, &group, &[]);
            }
            pass.dispatch_workgroups(x, y, 1);
        }
        if let Some(timer) = &mut self.timer {
            timer.resolve(encoder);
        }
        self.region = [region.width, region.height];
        self.readout = OcclusionReadout {
            enabled: true,
            refused: None,
            size: [size.0, size.1],
            levels: pyramid.levels,
            shared_depth: shared,
        };
    }

    /// The pyramid this view's selection tests against.
    pub fn occluders(&self) -> Option<Occluders<'_>> {
        self.pyramid.as_ref().map(|pyramid| Occluders {
            pyramid: &pyramid.all,
            region: self.region,
            levels: pyramid.levels,
        })
    }

    /// The last world view did not test.
    pub fn skipped(&mut self) {
        self.readout.enabled = false;
    }

    /// After the view's encoder was submitted.
    pub fn submitted(&mut self) {
        if self.readout.enabled {
            if let Some(timer) = &mut self.timer {
                timer.submitted();
            }
        }
    }

    pub fn timing(&self) -> GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| untimed(PASS), PassTimer::readout)
    }

    pub fn readout(&self) -> OcclusionReadout {
        self.readout.clone()
    }
}
