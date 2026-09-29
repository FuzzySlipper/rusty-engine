//! Composition pipelines (`compose.wgsl`): clearing one viewport of a shared
//! target, presenting an offscreen target into the primary output, and the
//! capture conversion to an encoded image. One pipeline set per colour format.

use crate::target::{ColorTarget, DEPTH_FORMAT, OFFSCREEN_FORMAT};
use crate::Gpu;

/// How a linear capture becomes its encoded image.
#[derive(Clone, Copy)]
pub(crate) struct Conversion {
    /// Source texels per output pixel on each axis.
    pub factor: u32,
    pub exposure: f32,
    pub aces: bool,
}

/// `Params` in compose.wgsl.
const PARAMS_BYTES: u64 = 48;

struct FormatPipelines {
    format: ColorTarget,
    clear_color_depth: wgpu::RenderPipeline,
    clear_depth: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
}

pub(crate) struct Compose {
    shader: wgpu::ShaderModule,
    blit_layout: wgpu::BindGroupLayout,
    convert_layout: wgpu::BindGroupLayout,
    clear_pipeline_layout: wgpu::PipelineLayout,
    blit_pipeline_layout: wgpu::PipelineLayout,
    convert_pipeline_layout: wgpu::PipelineLayout,
    clear_params: wgpu::Buffer,
    clear_bind_group: wgpu::BindGroup,
    formats: Vec<FormatPipelines>,
    convert: Option<wgpu::RenderPipeline>,
}

fn entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty,
        count: None,
    }
}

const PARAMS_ENTRY: wgpu::BindingType = wgpu::BindingType::Buffer {
    ty: wgpu::BufferBindingType::Uniform,
    has_dynamic_offset: false,
    min_binding_size: None,
};
const TEXTURE_ENTRY: wgpu::BindingType = wgpu::BindingType::Texture {
    sample_type: wgpu::TextureSampleType::Float { filterable: true },
    view_dimension: wgpu::TextureViewDimension::D2,
    multisampled: false,
};

fn params_bytes(color: [f32; 4], exposure: f32, aces: bool, factor: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(PARAMS_BYTES as usize);
    for value in color {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [exposure, if aces { 1.0 } else { 0.0 }, 0.0, 0.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [factor, 0, 0, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

impl Compose {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        let clear_layout = layout("render-wgpu compose clear", &[entry(0, PARAMS_ENTRY)]);
        let blit_layout = layout(
            "render-wgpu compose blit",
            &[
                entry(1, TEXTURE_ENTRY),
                entry(
                    2,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
            ],
        );
        let convert_layout = layout(
            "render-wgpu compose convert",
            &[entry(0, PARAMS_ENTRY), entry(1, TEXTURE_ENTRY)],
        );
        let pipeline_layout = |label, bind: &wgpu::BindGroupLayout| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(bind)],
                immediate_size: 0,
            })
        };
        let clear_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu compose clear"),
            size: PARAMS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let clear_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu compose clear"),
            layout: &clear_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: clear_params.as_entire_binding(),
            }],
        });
        Self {
            shader: device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("render-wgpu compose"),
                source: wgpu::ShaderSource::Wgsl(include_str!("compose.wgsl").into()),
            }),
            clear_pipeline_layout: pipeline_layout("render-wgpu compose clear", &clear_layout),
            blit_pipeline_layout: pipeline_layout("render-wgpu compose blit", &blit_layout),
            convert_pipeline_layout: pipeline_layout(
                "render-wgpu compose convert",
                &convert_layout,
            ),
            blit_layout,
            convert_layout,
            clear_params,
            clear_bind_group,
            formats: Vec::new(),
            convert: None,
        }
    }

    fn pipeline(
        &self,
        device: &wgpu::Device,
        label: &str,
        layout: &wgpu::PipelineLayout,
        fragment: &str,
        (target, samples): (wgpu::ColorTargetState, u32),
        depth: bool,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_fullscreen"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: depth.then_some(wgpu::DepthStencilState {
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
                module: &self.shader,
                entry_point: Some(fragment),
                compilation_options: Default::default(),
                targets: &[Some(target)],
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    fn format_index(&mut self, device: &wgpu::Device, format: ColorTarget) -> usize {
        if let Some(index) = self.formats.iter().position(|set| set.format == format) {
            return index;
        }
        let color = wgpu::ColorTargetState::from(format.format);
        let samples = format.samples;
        let depth_only = wgpu::ColorTargetState {
            write_mask: wgpu::ColorWrites::empty(),
            ..color.clone()
        };
        let set = FormatPipelines {
            format,
            clear_color_depth: self.pipeline(
                device,
                "render-wgpu clear viewport",
                &self.clear_pipeline_layout,
                "fs_clear",
                (color.clone(), samples),
                true,
            ),
            clear_depth: self.pipeline(
                device,
                "render-wgpu clear viewport depth",
                &self.clear_pipeline_layout,
                "fs_clear",
                (depth_only, samples),
                true,
            ),
            blit: self.pipeline(
                device,
                "render-wgpu present target",
                &self.blit_pipeline_layout,
                "fs_blit",
                (color, samples),
                false,
            ),
        };
        self.formats.push(set);
        self.formats.len() - 1
    }

    /// Make the viewport clear for `format` ready and set its colour.
    pub fn prepare_clear(&mut self, gpu: &Gpu, format: ColorTarget, color: [f32; 4]) {
        self.format_index(&gpu.device, format);
        gpu.queue
            .write_buffer(&self.clear_params, 0, &params_bytes(color, 1.0, false, 1));
    }

    /// Clear the pass's viewport: colour and depth, or depth only. Call
    /// [`Self::prepare_clear`] for the format first.
    pub fn clear_viewport(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
        color: bool,
    ) {
        let Some(set) = self.formats.iter().find(|set| set.format == format) else {
            return;
        };
        pass.set_pipeline(if color {
            &set.clear_color_depth
        } else {
            &set.clear_depth
        });
        pass.set_bind_group(0, &self.clear_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    pub fn blit_bind_group(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu present target"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Make the presentation blit for `format` ready.
    pub fn prepare_blit(&mut self, device: &wgpu::Device, format: ColorTarget) {
        self.format_index(device, format);
    }

    /// Draw a presented target over the pass's viewport. Call
    /// [`Self::prepare_blit`] for the format first.
    pub fn blit(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
        source: &wgpu::BindGroup,
    ) {
        let Some(set) = self.formats.iter().find(|set| set.format == format) else {
            return;
        };
        pass.set_pipeline(&set.blit);
        pass.set_bind_group(0, source, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Convert a linear capture of `factor`× the output size into `output`
    /// (an sRGB RGBA8 target): resolve, un-premultiply, expose or tone map.
    pub fn convert(
        &mut self,
        gpu: &Gpu,
        source: &wgpu::TextureView,
        output: &wgpu::TextureView,
        conversion: Conversion,
    ) {
        if self.convert.is_none() {
            self.convert = Some(self.pipeline(
                &gpu.device,
                "render-wgpu capture convert",
                &self.convert_pipeline_layout,
                "fs_convert",
                (wgpu::ColorTargetState::from(OFFSCREEN_FORMAT), 1),
                false,
            ));
        }
        use wgpu::util::DeviceExt;
        let params = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("render-wgpu capture convert"),
                contents: &params_bytes(
                    [0.0; 4],
                    conversion.exposure,
                    conversion.aces,
                    conversion.factor,
                ),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu capture convert"),
            layout: &self.convert_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(source),
                },
            ],
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu capture convert"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu capture convert"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(pipeline) = &self.convert {
                pass.set_pipeline(pipeline);
            }
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
    }
}
