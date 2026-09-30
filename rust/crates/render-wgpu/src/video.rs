//! Video playback: the one active clip, decoded by `render-video` on the
//! Engine presentation timeline and drawn over the primary target, letterboxed
//! on black, covering the view.
//!
//! A clip starts at the timeline position its play op was applied at, drawn
//! or not, so a held simulation holds the picture, a viewer that attaches
//! mid-clip sees it where the Engine (and its soundtrack) is, and a rebuilt
//! renderer, whose baseline applies at the current time, plays an active clip
//! from its start. Playback reports how it ends: `Completed`
//! at the clip's end, `Skipped` for a skip op, `Failed` when the clip cannot
//! be read or decoded. A stop ends it silently.

use std::sync::Arc;

use render_presentation::{VideoPlaybackHandle, VideoProjectionOp};
use render_video::{VideoClip, VideoPlayback};

use crate::target::TargetView;
use crate::{Renderer, ResourceSource};

/// How a playback ended, for the runtime's video realization feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFact {
    Completed {
        handle: VideoPlaybackHandle,
    },
    Skipped {
        handle: VideoPlaybackHandle,
    },
    Failed {
        handle: VideoPlaybackHandle,
        failure: VideoFailure,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFailure {
    /// The clip is not an admitted WebM/VP9 clip, or a frame did not decode.
    DecodeFailed,
    /// The clip's bytes were not available to the renderer.
    HostFailure,
}

pub(crate) struct Video {
    active: Option<Active>,
    facts: Vec<VideoFact>,
    layout: wgpu::BindGroupLayout,
    shader: wgpu::ShaderModule,
    sampler: wgpu::Sampler,
    params: wgpu::Buffer,
    pipelines: Vec<((wgpu::TextureFormat, u32), wgpu::RenderPipeline)>,
}

struct Active {
    handle: VideoPlaybackHandle,
    playback: VideoPlayback,
    /// The timeline position of the first frame: when the play op applied.
    started_at: f64,
    planes: Option<Planes>,
}

struct Planes {
    textures: [wgpu::Texture; 3],
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
}

/// The uniform: the video rectangle in target pixels, then whether the
/// target encodes sRGB (the shader writes linear light to one).
const PARAMS_BYTES: u64 = 32;

impl Video {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let plane = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu video"),
            entries: &[
                plane(0),
                plane(1),
                plane(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        Self {
            active: None,
            facts: Vec::new(),
            shader: device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("render-wgpu video"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            }),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu video"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu video"),
                size: PARAMS_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            layout,
            pipelines: Vec::new(),
        }
    }

    fn end(&mut self, fact: Option<VideoFact>) {
        self.active = None;
        self.facts.extend(fact);
    }
}

impl Renderer {
    /// Apply one video op from a presentation delta.
    pub(crate) fn apply_video_op(
        &mut self,
        op: &VideoProjectionOp,
        resources: &dyn ResourceSource,
    ) {
        match op {
            VideoProjectionOp::Play { handle, clip } => {
                // A new play replaces the active playback without a fact.
                self.video.active = None;
                let Some(bytes) = resources.bytes(&clip.asset) else {
                    self.video.end(Some(VideoFact::Failed {
                        handle: *handle,
                        failure: VideoFailure::HostFailure,
                    }));
                    return;
                };
                match VideoClip::open(&bytes) {
                    Ok(clip) => {
                        self.video.active = Some(Active {
                            handle: *handle,
                            playback: VideoPlayback::new(Arc::clone(&clip)),
                            started_at: self.animation_time,
                            planes: None,
                        });
                    }
                    Err(_) => self.video.end(Some(VideoFact::Failed {
                        handle: *handle,
                        failure: VideoFailure::DecodeFailed,
                    })),
                }
            }
            VideoProjectionOp::Stop { handle } => {
                if self.active_video() == Some(*handle) {
                    self.video.end(None);
                }
            }
            VideoProjectionOp::Skip { handle } => match self.active_video() {
                Some(active) if active == *handle => {
                    self.video.end(Some(VideoFact::Skipped { handle: *handle }));
                }
                Some(_) => {}
                // A skip with nothing playing (it failed or ended first) is
                // still reported.
                None => self
                    .video
                    .facts
                    .push(VideoFact::Skipped { handle: *handle }),
            },
        }
    }

    /// The playback handle on screen, if a clip is playing.
    pub fn active_video(&self) -> Option<VideoPlaybackHandle> {
        self.video.active.as_ref().map(|active| active.handle)
    }

    /// How playbacks ended since the last call, oldest first.
    pub fn take_video_facts(&mut self) -> Vec<VideoFact> {
        std::mem::take(&mut self.video.facts)
    }

    /// End the playing clip if Engine time has passed its end, without
    /// decoding anything.
    pub(crate) fn end_finished_video(&mut self) {
        let Some(active) = &self.video.active else {
            return;
        };
        let position = (self.animation_time - active.started_at).max(0.0);
        if active.playback.finished(position) {
            let handle = active.handle;
            self.video.end(Some(VideoFact::Completed { handle }));
        }
    }

    /// Decode up to the current Engine time and upload the frame shown then.
    pub(crate) fn advance_video(&mut self) {
        let Some(active) = &mut self.video.active else {
            return;
        };
        let position = (self.animation_time - active.started_at).max(0.0);
        let handle = active.handle;
        if active.playback.finished(position) {
            self.video.end(Some(VideoFact::Completed { handle }));
            return;
        }
        if active.playback.advance(position).is_err() {
            self.video.end(Some(VideoFact::Failed {
                handle,
                failure: VideoFailure::DecodeFailed,
            }));
            return;
        }
        let Some((frame, true)) = active.playback.take_frame() else {
            return;
        };
        let device = &self.gpu.device;
        let size = (frame.width, frame.height);
        if active.planes.as_ref().map(|planes| planes.size) != Some(size) {
            let plane = |width: u32, height: u32| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("render-wgpu video plane"),
                    size: crate::target::extent(width, height),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                })
            };
            let (chroma_width, chroma_height) = (size.0.div_ceil(2), size.1.div_ceil(2));
            let textures = [
                plane(size.0, size.1),
                plane(chroma_width, chroma_height),
                plane(chroma_width, chroma_height),
            ];
            let views = textures
                .each_ref()
                .map(|texture| texture.create_view(&Default::default()));
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu video"),
                layout: &self.video.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&views[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&views[1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&views[2]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.video.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.video.params.as_entire_binding(),
                    },
                ],
            });
            active.planes = Some(Planes {
                textures,
                bind_group,
                size,
            });
        }
        let planes = active.planes.as_ref().expect("planes were just made");
        for (index, texture) in planes.textures.iter().enumerate() {
            let extent = texture.size();
            self.gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &frame.planes[index],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(frame.strides[index] as u32),
                    rows_per_image: None,
                },
                extent,
            );
        }
    }

    /// Draw the playing clip over the whole primary target, letterboxed, and
    /// report whether a picture covered it.
    pub(crate) fn draw_video(&mut self, target: &TargetView<'_>) -> bool {
        let Some(Active {
            planes: Some(planes),
            ..
        }) = &self.video.active
        else {
            return false;
        };
        let (width, height) = (target.width as f32, target.height as f32);
        let (video_width, video_height) = (planes.size.0 as f32, planes.size.1 as f32);
        let scale = (width / video_width).min(height / video_height);
        let (shown_width, shown_height) = (video_width * scale, video_height * scale);
        let left = (width - shown_width) * 0.5;
        let top = (height - shown_height) * 0.5;
        let srgb = if target.format.is_srgb() { 1.0f32 } else { 0.0 };
        let params: Vec<u8> = [
            left,
            top,
            left + shown_width,
            top + shown_height,
            srgb,
            0.0,
            0.0,
            0.0,
        ]
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
        self.gpu.queue.write_buffer(&self.video.params, 0, &params);
        // Over the finished frame: the resolved image of a multisampled
        // target.
        let (view, samples) = match target.resolve {
            Some(resolve) => (resolve, 1),
            None => (target.color, target.samples),
        };
        let key = (target.format, samples);
        let device = &self.gpu.device;
        let index = match self.video.pipelines.iter().position(|(k, _)| *k == key) {
            Some(index) => index,
            None => {
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("render-wgpu video"),
                    bind_group_layouts: &[Some(&self.video.layout)],
                    immediate_size: 0,
                });
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("render-wgpu video"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &self.video.shader,
                        entry_point: Some("vs_video"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: samples,
                        ..Default::default()
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &self.video.shader,
                        entry_point: Some("fs_video"),
                        compilation_options: Default::default(),
                        targets: &[Some(target.format.into())],
                    }),
                    multiview_mask: None,
                    cache: None,
                });
                self.video.pipelines.push((key, pipeline));
                self.video.pipelines.len() - 1
            }
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu video"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu video"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.video.pipelines[index].1);
            pass.set_bind_group(0, &planes.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.gpu.queue.submit([encoder.finish()]);
        true
    }
}

/// BT.601 limited-range YUV to RGB: the products' clips carry no colour
/// description, and browsers assume the same for standard-definition VP9.
const SHADER: &str = r#"
@group(0) @binding(0) var y_plane: texture_2d<f32>;
@group(0) @binding(1) var u_plane: texture_2d<f32>;
@group(0) @binding(2) var v_plane: texture_2d<f32>;
@group(0) @binding(3) var planes: sampler;
struct Params { rect: vec4<f32>, flags: vec4<f32> };
@group(0) @binding(4) var<uniform> params: Params;

@vertex
fn vs_video(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

fn decode(encoded: vec3<f32>) -> vec3<f32> {
    let low = encoded / 12.92;
    let high = pow((encoded + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, encoded <= vec3<f32>(0.04045));
}

@fragment
fn fs_video(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let rect = params.rect;
    if (position.x < rect.x || position.y < rect.y || position.x >= rect.z || position.y >= rect.w) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let uv = (position.xy - rect.xy) / (rect.zw - rect.xy);
    // The planes have one mip level. An explicit level keeps the samples out
    // of WGSL's uniform-control-flow rule after the early return, which
    // browsers enforce (#8874).
    let y = (textureSampleLevel(y_plane, planes, uv, 0.0).r - 16.0 / 255.0) * (255.0 / 219.0);
    let u = (textureSampleLevel(u_plane, planes, uv, 0.0).r - 128.0 / 255.0) * (255.0 / 224.0);
    let v = (textureSampleLevel(v_plane, planes, uv, 0.0).r - 128.0 / 255.0) * (255.0 / 224.0);
    var rgb = clamp(vec3<f32>(
        y + 1.402 * v,
        y - 0.344136 * u - 0.714136 * v,
        y + 1.772 * u,
    ), vec3<f32>(0.0), vec3<f32>(1.0));
    if (params.flags.x > 0.5) {
        rgb = decode(rgb);
    }
    return vec4<f32>(rgb, 1.0);
}
"#;
