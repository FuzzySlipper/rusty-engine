//! Occlusion of a world view's opaque parts and selected scatter copies
//! behind its opaque scene (#10020, #10698, `hiz.wgsl`).
//!
//! With `RendererOptions::occlusion_culling`, a world view first draws its
//! occluders depth-only at its world pass's size and samples per pixel:
//! opaque parts other than scatter copies whose materials keep every sample
//! they cover (none masked, none with a product caster stage that may
//! discard). Sampling as the world pass does, the depth leaves open every
//! opening a world-pass sample sees through. That depth, each pixel's
//! farthest sample, is reduced to a farthest-depth pyramid. The GPU cull
//! (`culling.rs`) drops an opaque part whose bounds lie wholly behind it,
//! and the scatter selection (`scatter_select.rs`) a copy.
//!
//! Where the cull runs, the occluders are the parts its last cull found
//! visible, which it selects itself; where it cannot, they are every opaque
//! part of the CPU list and only the copies are tested. Either way the
//! depth is drawn at this frame's camera, so camera motion, rebases and
//! edits need nothing more, and nothing the tests keep is missing for a
//! frame. Scatter copies never occlude, and the shadows' lists are not
//! tested.

use crate::frame::PixelRect;
use crate::target::DEPTH_FORMAT;
use crate::timing::{untimed, GpuPassTiming, PassTimer};
use crate::Gpu;

/// The pyramid's reduction; the depth pass is timed on its own.
const PASS: &str = "occlusion";
const DEPTH_PASS: &str = "occlusion-depth";
const WORKGROUP: u32 = 8;
/// `tile` in hiz.wgsl: a workgroup's texels of its reduction's first level.
const SHARED_BYTES: u32 = WORKGROUP * WORKGROUP * 4;
/// The levels a reduction writes: as many as an 8×8 tile reduces to.
const LEVELS_PER_REDUCTION: u32 = 4;
/// `Params` in hiz.wgsl.
const PARAMS_BYTES: u64 = 32;
/// Each dispatch's params at its own offset: the most any device asks
/// uniform offsets to align to.
const PARAMS_STRIDE: u64 = 256;
/// `Hiz` in hiz_common.wgsl: its size, levels and each level's start, the
/// most levels a pyramid can have, then the depth.
const MAX_LEVELS: u32 = 16;
const HEADER_BYTES: u64 = 16 + MAX_LEVELS as u64 * 4;
/// The smallest pyramid buffer a reader binds: the header and one texel,
/// rounded to the struct's 8-byte alignment.
pub(crate) const MIN_PYRAMID_BYTES: u64 = HEADER_BYTES + 8;
#[cfg(not(test))]
const PYRAMID_USAGE: wgpu::BufferUsages =
    wgpu::BufferUsages::STORAGE.union(wgpu::BufferUsages::COPY_DST);
/// The tests read the levels back.
#[cfg(test)]
const PYRAMID_USAGE: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE
    .union(wgpu::BufferUsages::COPY_DST)
    .union(wgpu::BufferUsages::COPY_SRC);

/// The occlusion test of the last world view, for diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OcclusionReadout {
    /// The last world view tested against its depth.
    pub enabled: bool,
    /// It tested its opaque parts too, not only its selected copies.
    pub parts: bool,
    /// Why this device cannot; `None` while it can.
    pub refused: Option<String>,
    /// The pyramid's level-0 size (the view's region, padded to powers of
    /// two) and its levels.
    pub size: [u32; 2],
    pub levels: u32,
    /// Samples per pixel of its depth (the world pass's).
    pub samples: u32,
}

/// What the selection tests against: the pyramid and the size of the
/// view's region in its level-0 texels, and its levels.
pub(crate) struct Occluders<'a> {
    pub pyramid: &'a wgpu::Buffer,
    pub region: [u32; 2],
    pub levels: u32,
}

/// The pyramid's pipelines (`hiz.wgsl`), which the splats' occlusion test
/// builds its own pyramid with (`splats.rs`).
pub(crate) struct Pipelines {
    seed_layout: wgpu::BindGroupLayout,
    seed_samples_layout: wgpu::BindGroupLayout,
    reduce_layout: wgpu::BindGroupLayout,
    seed: wgpu::ComputePipeline,
    seed_samples: wgpu::ComputePipeline,
    reduce: wgpu::ComputePipeline,
}

/// A pyramid's buffer (`Hiz` in hiz_common.wgsl) with the groups that
/// build it, made once for its size.
struct Pyramid {
    size: (u32, u32),
    levels: u32,
    buffer: wgpu::Buffer,
    /// Each dispatch's params, a slot each: the seed's, then each
    /// reduction's.
    params: wgpu::Buffer,
    /// What the params hold: the region and the block side.
    written: Option<(PixelRect, u32)>,
    /// Each reduction's first level and group.
    reductions: Vec<(u32, wgpu::BindGroup)>,
    /// The seed's group, for the depth it reads.
    seed: Option<(wgpu::TextureView, wgpu::BindGroup)>,
}

/// A depth texture's region reduced to a farthest-depth pyramid
/// (`hiz.wgsl`): level 0 the region, padded with the far plane to powers of
/// two, each level above the farthest of the four texels under it. The
/// levels lie in one storage buffer, which the tests read whole, rather
/// than a texture's mips, which wgpu tracks one by one. The seed writes
/// level 0 and each reduction four levels above it, so a 1080p view's
/// pyramid takes four dispatches.
pub(crate) struct DepthPyramid {
    pyramid: Option<Pyramid>,
    region: [u32; 2],
}

impl DepthPyramid {
    pub fn new() -> Self {
        Self {
            pyramid: None,
            region: [0, 0],
        }
    }

    /// Reduce `depth` (with `samples` per pixel) over `region`, a level-0
    /// texel the farthest of a `block` by `block` square of its pixels,
    /// timed by `timer` when given; returns the pyramid's level-0 size and
    /// levels.
    #[allow(clippy::too_many_arguments, reason = "one reduction's inputs")]
    pub fn build(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &Pipelines,
        (depth, samples): (&wgpu::TextureView, u32),
        region: PixelRect,
        block: u32,
        timer: Option<&PassTimer>,
    ) -> ([u32; 2], u32) {
        let size = (
            region.width.div_ceil(block).max(1).next_power_of_two(),
            region.height.div_ceil(block).max(1).next_power_of_two(),
        );
        if self
            .pyramid
            .as_ref()
            .is_none_or(|pyramid| pyramid.size != size)
        {
            self.pyramid = Some(Pyramid::new(gpu, pipelines, size));
        }
        let pyramid = self.pyramid.as_mut().expect("made above");
        if pyramid.written != Some((region, block)) {
            pyramid.write_params(gpu, region, block);
        }
        let multisampled = samples > 1;
        if pyramid
            .seed
            .as_ref()
            .is_none_or(|(seeded, _)| seeded != depth)
        {
            let (layout, depth_binding) = if multisampled {
                (&pipelines.seed_samples_layout, 5)
            } else {
                (&pipelines.seed_layout, 1)
            };
            let group = pyramid_group(
                gpu,
                layout,
                &pyramid.params,
                0,
                &pyramid.buffer,
                Some((depth_binding, depth)),
            );
            pyramid.seed = Some((depth.clone(), group));
        }
        let groups = |level: u32| {
            (
                (size.0 >> level).max(1).div_ceil(WORKGROUP),
                (size.1 >> level).max(1).div_ceil(WORKGROUP),
            )
        };
        // One pass: each dispatch reads the level below its first, which
        // the dispatch before it wrote.
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(PASS),
            timestamp_writes: timer.and_then(PassTimer::compute_writes),
        });
        pass.set_pipeline(if multisampled {
            &pipelines.seed_samples
        } else {
            &pipelines.seed
        });
        pass.set_bind_group(0, &pyramid.seed.as_ref().expect("made above").1, &[]);
        let (x, y) = groups(0);
        pass.dispatch_workgroups(x, y, 1);
        if !pyramid.reductions.is_empty() {
            pass.set_pipeline(&pipelines.reduce);
        }
        for (first, group) in &pyramid.reductions {
            let (x, y) = groups(*first);
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(x, y, 1);
        }
        drop(pass);
        self.region = [region.width, region.height];
        ([size.0, size.1], pyramid.levels)
    }

    /// The pyramid, the region's size in its level-0 texels, and its
    /// levels.
    pub fn occluders(&self) -> Option<Occluders<'_>> {
        self.pyramid.as_ref().map(|pyramid| Occluders {
            pyramid: &pyramid.buffer,
            region: self.region,
            levels: pyramid.levels,
        })
    }

    /// Let go of the depth the seed last read, which its group holds, when
    /// no view will build from it.
    pub fn release_depth(&mut self) {
        if let Some(pyramid) = &mut self.pyramid {
            pyramid.seed = None;
        }
    }
}

impl Pyramid {
    fn new(gpu: &Gpu, pipelines: &Pipelines, size: (u32, u32)) -> Self {
        let device = &gpu.device;
        let levels = size.0.max(size.1).ilog2() + 1;
        let mut header = Vec::with_capacity(HEADER_BYTES as usize);
        for value in [size.0, size.1, levels, 0] {
            header.extend_from_slice(&value.to_le_bytes());
        }
        let mut texels = 0u64;
        for level in 0..MAX_LEVELS {
            header.extend_from_slice(&(texels as u32).to_le_bytes());
            if level < levels {
                texels += u64::from((size.0 >> level).max(1)) * u64::from((size.1 >> level).max(1));
            }
        }
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu occlusion pyramid"),
            size: (HEADER_BYTES + texels * 4).max(MIN_PYRAMID_BYTES),
            usage: PYRAMID_USAGE,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&buffer, 0, &header);
        let firsts: Vec<u32> = (1..levels).step_by(LEVELS_PER_REDUCTION as usize).collect();
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu occlusion pyramid params"),
            size: (firsts.len() as u64 + 1) * PARAMS_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let reductions = firsts
            .iter()
            .enumerate()
            .map(|(index, &first)| {
                let group = pyramid_group(
                    gpu,
                    &pipelines.reduce_layout,
                    &params,
                    (index as u64 + 1) * PARAMS_STRIDE,
                    &buffer,
                    None,
                );
                (first, group)
            })
            .collect();
        Self {
            size,
            levels,
            buffer,
            params,
            written: None,
            reductions,
            seed: None,
        }
    }

    /// Each dispatch's params for `region` and `block`: the seed's, then
    /// each reduction's with the levels it writes.
    fn write_params(&mut self, gpu: &Gpu, region: PixelRect, block: u32) {
        let dispatches = std::iter::once((0, 1)).chain(
            self.reductions
                .iter()
                .map(|(first, _)| (*first, (self.levels - first).min(LEVELS_PER_REDUCTION))),
        );
        let mut params = Vec::new();
        for (first, levels) in dispatches {
            let start = params.len();
            for value in [
                region.x,
                region.y,
                region.width,
                region.height,
                block,
                levels,
                first,
            ] {
                params.extend_from_slice(&value.to_le_bytes());
            }
            params.resize(start + PARAMS_STRIDE as usize, 0);
        }
        gpu.queue.write_buffer(&self.params, 0, &params);
        self.written = Some((region, block));
    }
}

/// A test's binding of the pyramid (`Hiz` in hiz_common.wgsl), read-only.
pub(crate) fn pyramid_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(MIN_PYRAMID_BYTES),
        },
        count: None,
    }
}

/// Bound in place of a pyramid where a test has none; it never reads it.
pub(crate) fn no_pyramid(device: &wgpu::Device, label: &str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: MIN_PYRAMID_BYTES,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    })
}

/// A dispatch's group: its params at `offset`, the pyramid, and for the
/// seed the depth it reads at its binding.
fn pyramid_group(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    params: &wgpu::Buffer,
    offset: u64,
    pyramid: &wgpu::Buffer,
    depth: Option<(u32, &wgpu::TextureView)>,
) -> wgpu::BindGroup {
    let mut entries = vec![
        wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: params,
                offset,
                size: wgpu::BufferSize::new(PARAMS_BYTES),
            }),
        },
        wgpu::BindGroupEntry {
            binding: 2,
            resource: pyramid.as_entire_binding(),
        },
    ];
    if let Some((binding, view)) = depth {
        entries.push(wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        });
    }
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("render-wgpu occlusion pyramid"),
        layout,
        entries: &entries,
    })
}

pub(crate) struct Occlusion {
    pipelines: Result<Pipelines, String>,
    pyramid: DepthPyramid,
    /// The views' occluder depth: its size, samples per pixel and view.
    depth: Option<((u32, u32), u32, wgpu::TextureView)>,
    timer: Option<PassTimer>,
    depth_timer: Option<PassTimer>,
    readout: OcclusionReadout,
}

impl Occlusion {
    pub fn new(gpu: &Gpu, shader: wgpu::ShaderModule) -> Self {
        let device = &gpu.device;
        let pipelines = match gpu.compute_refusal([WORKGROUP, WORKGROUP, 1], SHARED_BYTES) {
            Some(refused) => Err(refused),
            None => {
                let texture = |binding, sample_type, multisampled| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled,
                    },
                    count: None,
                };
                // The params, the pyramid, and for the seed the depth it
                // reads.
                let layout = |label, depth: Option<wgpu::BindGroupLayoutEntry>| {
                    let mut entries = vec![
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(PARAMS_BYTES),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(MIN_PYRAMID_BYTES),
                            },
                            count: None,
                        },
                    ];
                    entries.extend(depth);
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some(label),
                        entries: &entries,
                    })
                };
                let seed_layout = layout(
                    "render-wgpu occlusion seed",
                    Some(texture(1, wgpu::TextureSampleType::Depth, false)),
                );
                let seed_samples_layout = layout(
                    "render-wgpu occlusion seed",
                    Some(texture(5, wgpu::TextureSampleType::Depth, true)),
                );
                let reduce_layout = layout("render-wgpu occlusion reduce", None);
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
                    seed_samples: pipeline(&seed_samples_layout, "cs_seed_samples"),
                    reduce: pipeline(&reduce_layout, "cs_reduce"),
                    seed_layout,
                    seed_samples_layout,
                    reduce_layout,
                })
            }
        };
        Self {
            pyramid: DepthPyramid::new(),
            readout: OcclusionReadout {
                refused: pipelines.as_ref().err().cloned(),
                ..Default::default()
            },
            pipelines,
            depth: None,
            timer: PassTimer::new(gpu, PASS),
            depth_timer: PassTimer::new(gpu, DEPTH_PASS),
        }
    }

    pub fn available(&self) -> bool {
        self.pipelines.is_ok()
    }

    /// Ready the view's occluder depth for a world pass drawing a target of
    /// `size` at `samples` per pixel.
    pub fn prepare_depth(&mut self, gpu: &Gpu, size: (u32, u32), samples: u32) {
        for timer in [&mut self.timer, &mut self.depth_timer]
            .into_iter()
            .flatten()
        {
            timer.collect(gpu);
        }
        let size = (size.0.max(1), size.1.max(1));
        if self
            .depth
            .as_ref()
            .is_none_or(|(at, count, _)| (*at, *count) != (size, samples))
        {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("render-wgpu occlusion depth"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.depth = Some((size, samples, texture.create_view(&Default::default())));
        }
    }

    /// The view's own occluder depth pass over `region`
    /// ([`Self::prepare_depth`]): `draw` encodes the occluders into it. It
    /// is timed as `occlusion-depth`.
    pub fn encode_depth(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        region: PixelRect,
        draw: impl FnOnce(&mut wgpu::RenderPass<'_>),
    ) {
        let (_, _, depth) = self.depth.as_ref().expect("prepare_depth made it");
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
            timestamp_writes: self.depth_timer.as_ref().and_then(PassTimer::render_writes),
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

    /// Reduce the view's occluder depth over `region` (its viewport) to the
    /// pyramid the tests read; `parts`: the cull tests the opaque parts
    /// against it. Times it as `occlusion`, after resolving the depth
    /// pass's time.
    pub fn build(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        region: PixelRect,
        parts: bool,
    ) {
        if let Some(timer) = &mut self.depth_timer {
            timer.resolve(encoder);
        }
        let Ok(pipelines) = &self.pipelines else {
            return;
        };
        let (_, samples, depth) = self.depth.as_ref().expect("prepare_depth made it");
        let (samples, depth) = (*samples, depth.clone());
        let (size, levels) = self.pyramid.build(
            gpu,
            encoder,
            pipelines,
            (&depth, samples),
            region,
            1,
            self.timer.as_ref(),
        );
        if let Some(timer) = &mut self.timer {
            timer.resolve(encoder);
        }
        self.readout = OcclusionReadout {
            enabled: true,
            parts,
            refused: None,
            size,
            levels,
            samples,
        };
    }

    /// The pyramid this view's tests read.
    pub fn occluders(&self) -> Option<Occluders<'_>> {
        self.pyramid.occluders()
    }

    /// The pyramid's pipelines, on a device that builds one.
    pub fn pipelines(&self) -> Option<&Pipelines> {
        self.pipelines.as_ref().ok()
    }

    /// The last world view did not test: its passes report no time.
    pub fn skipped(&mut self) {
        self.readout.enabled = false;
        for timer in [&mut self.timer, &mut self.depth_timer]
            .into_iter()
            .flatten()
        {
            timer.restart();
        }
    }

    /// After the view's encoder was submitted.
    pub fn submitted(&mut self) {
        if self.readout.enabled {
            for timer in [&mut self.timer, &mut self.depth_timer]
                .into_iter()
                .flatten()
            {
                timer.submitted();
            }
        }
    }

    /// The depth pass's time, then the pyramid's.
    pub fn timings(&self) -> [GpuPassTiming; 2] {
        [
            self.depth_timer
                .as_ref()
                .map_or_else(|| untimed(DEPTH_PASS), PassTimer::readout),
            self.timer
                .as_ref()
                .map_or_else(|| untimed(PASS), PassTimer::readout),
        ]
    }

    pub fn readout(&self) -> OcclusionReadout {
        self.readout.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaders::{Entry, Features, Shaders};

    /// A depth of `size` at `samples` per pixel, each pixel a different
    /// depth, every sample of a pixel the same.
    fn depth(gpu: &Gpu, size: (u32, u32), samples: u32) -> wgpu::Texture {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(
                "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
                    return vec4<f32>(uv * 2.0 - 1.0, 0.5, 1.0);
                }
                struct Out { @builtin(frag_depth) depth: f32 };
                @fragment fn fs(@builtin(position) at: vec4<f32>) -> Out {
                    let p = vec2<u32>(at.xy);
                    var h = p.x * 73856093u ^ p.y * 19349663u;
                    h = (h ^ (h >> 13u)) * 1274126177u;
                    return Out(f32(h >> 8u) / 16777216.0);
                }"
                .into(),
            ),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
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
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        let view = texture.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
        drop(pass);
        gpu.queue.submit([encoder.finish()]);
        texture
    }

    /// `texture`'s mip `level` (`aspect` of it) as f32s, row by row.
    fn read(
        gpu: &Gpu,
        texture: &wgpu::Texture,
        level: u32,
        aspect: wgpu::TextureAspect,
    ) -> Vec<f32> {
        let (width, height) = (
            (texture.width() >> level).max(1),
            (texture.height() >> level).max(1),
        );
        let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([encoder.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let mapped = buffer.slice(..).get_mapped_range().expect("mapped");
        (0..height)
            .flat_map(|y| {
                let start = (y * row) as usize;
                bytemuck::cast_slice::<u8, f32>(&mapped[start..start + (width * 4) as usize])
                    .to_vec()
            })
            .collect()
    }

    /// `buffer`'s bytes.
    fn read_buffer(gpu: &Gpu, buffer: &wgpu::Buffer) -> Vec<u8> {
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: buffer.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffer, 0, &readback, 0, None);
        gpu.queue.submit([encoder.finish()]);
        readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let mapped = readback.slice(..).get_mapped_range().expect("mapped");
        mapped.to_vec()
    }

    /// The pyramid as `hiz.wgsl` describes it, from `depth` (`width` wide).
    fn reference(
        depth: &[f32],
        width: u32,
        region: PixelRect,
        block: u32,
        size: (u32, u32),
    ) -> Vec<Vec<f32>> {
        let levels = size.0.max(size.1).ilog2() + 1;
        let mut level_zero = Vec::new();
        for y in 0..size.1 {
            for x in 0..size.0 {
                let (first_x, first_y) = (x * block, y * block);
                let mut value = 1.0f32;
                if first_x < region.width && first_y < region.height {
                    value = 0.0;
                    for py in first_y..(first_y + block).min(region.height) {
                        for px in first_x..(first_x + block).min(region.width) {
                            let at = (region.y + py) * width + region.x + px;
                            value = value.max(depth[at as usize]);
                        }
                    }
                }
                level_zero.push(value);
            }
        }
        let mut pyramid = vec![level_zero];
        for level in 1..levels {
            let below = pyramid.last().expect("level 0");
            let below_size = (
                (size.0 >> (level - 1)).max(1),
                (size.1 >> (level - 1)).max(1),
            );
            let above_size = ((size.0 >> level).max(1), (size.1 >> level).max(1));
            let texel = |x: u32, y: u32| {
                below[(y.min(below_size.1 - 1) * below_size.0 + x.min(below_size.0 - 1)) as usize]
            };
            let mut above = Vec::new();
            for y in 0..above_size.1 {
                for x in 0..above_size.0 {
                    above.push(
                        texel(2 * x, 2 * y)
                            .max(texel(2 * x + 1, 2 * y))
                            .max(texel(2 * x, 2 * y + 1))
                            .max(texel(2 * x + 1, 2 * y + 1)),
                    );
                }
            }
            pyramid.push(above);
        }
        pyramid
    }

    /// Every level of the pyramid holds exactly the farthest depth under
    /// it, as reduced a level at a time: for regions square and not,
    /// offset in their depth, levels left over past a dispatch's four, and
    /// blocks of one pixel and four, with one sample a pixel and four.
    #[test]
    fn each_level_is_the_farthest_depth_under_it() {
        let (_turn, gpu) = crate::gpu::test_device::shared();
        let shader = Shaders::new()
            .module(&gpu.device, Entry::Hiz, Features::default())
            .expect("hiz.wgsl");
        let occlusion = Occlusion::new(&gpu, shader);
        let Some(pipelines) = occlusion.pipelines() else {
            eprintln!("skipped: {:?}", occlusion.readout().refused);
            return;
        };
        let cases = [
            // (depth size, region, block, samples)
            ((70, 45), (3, 5, 61, 37), 1, 1),
            ((310, 50), (4, 6, 300, 40), 1, 1),
            ((310, 50), (4, 6, 300, 40), 4, 1),
            ((310, 50), (4, 6, 300, 40), 4, 4),
            ((40, 300), (0, 0, 40, 300), 1, 4),
            ((5, 3), (0, 0, 5, 3), 1, 1),
            ((640, 360), (0, 0, 640, 360), 1, 1),
        ];
        for (size, (x, y, width, height), block, samples) in cases {
            let region = PixelRect {
                x,
                y,
                width,
                height,
            };
            let single = depth(&gpu, size, 1);
            let drawn = if samples > 1 {
                depth(&gpu, size, samples)
            } else {
                single.clone()
            };
            let values = read(&gpu, &single, 0, wgpu::TextureAspect::DepthOnly);
            let mut pyramid = DepthPyramid::new();
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            let (built, levels) = pyramid.build(
                &gpu,
                &mut encoder,
                pipelines,
                (&drawn.create_view(&Default::default()), samples),
                region,
                block,
                None,
            );
            gpu.queue.submit([encoder.finish()]);
            let expected = reference(&values, size.0, region, block, (built[0], built[1]));
            assert_eq!(levels as usize, expected.len());
            let buffer = &pyramid.pyramid.as_ref().expect("built").buffer;
            let words = read_buffer(&gpu, buffer);
            let header: &[u32] = bytemuck::cast_slice(&words[..HEADER_BYTES as usize]);
            assert_eq!(&header[..3], &[built[0], built[1], levels]);
            let depth: &[f32] = bytemuck::cast_slice(&words[HEADER_BYTES as usize..]);
            for (level, expected) in expected.iter().enumerate() {
                let start = header[4 + level] as usize;
                let got = &depth[start..start + expected.len()];
                assert!(
                    got.iter()
                        .zip(expected)
                        .all(|(a, b)| a.to_bits() == b.to_bits()),
                    "{size:?} {region:?} block {block} samples {samples}: level {level} differs"
                );
            }
        }
    }
}
