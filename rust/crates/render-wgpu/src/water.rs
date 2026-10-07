//! The water surfaces' view of the scene (`Features::WATER`): the opaque
//! pass's depth copied into a single-sample depth texture before the blend
//! pass, bound as group 2 of the water pipelines (`Layouts::water`, in place
//! of the view's occlusion, which blended parts never read), so a water
//! material reads what lies behind its surface
//! (`world.wgsl` `water_surface`). A view copies only when a water material
//! is among its blended parts; other views draw as before.

use std::collections::HashMap;

use render_shaders::{Entry, Features};

use crate::gpu::Gpu;
use crate::pipelines::standard;
use crate::shaders::Shaders;
use crate::target::{TargetView, DEPTH_FORMAT};

/// The copy, its bind group for the water pipelines and the copy pipelines
/// per sample count of the source.
pub(crate) struct WaterDepth {
    shader: wgpu::ShaderModule,
    /// Group 2 of the water pipelines: the copy.
    layout: wgpu::BindGroupLayout,
    /// Per source sample count: the source's layout and the copy pipeline.
    copies: HashMap<u32, (wgpu::BindGroupLayout, wgpu::RenderPipeline)>,
    /// The copy for the last target size, and its bind group.
    copy: Option<Copy>,
}

struct Copy {
    size: (u32, u32),
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl WaterDepth {
    pub fn new(device: &wgpu::Device, shaders: &mut Shaders) -> Self {
        let shader = standard(shaders.module(device, Entry::WaterDepth, Features::default()));
        Self {
            shader,
            // The same layout `Layouts` gives the water pipelines' group 3.
            layout: device.create_bind_group_layout(&layout_descriptor()),
            copies: HashMap::new(),
            copy: None,
        }
    }

    /// Whether a blended draw list holds a water material.
    pub fn wanted(features: impl Iterator<Item = Features>) -> bool {
        let mut features = features;
        features.any(|features| features.contains(Features::WATER))
    }

    /// Copy `target`'s depth (the opaque pass done) for the water surfaces
    /// the blend pass draws next.
    pub fn capture(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &TargetView<'_>,
    ) {
        let size = (target.width, target.height);
        if self.copy.as_ref().is_none_or(|copy| copy.size != size) {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("render-wgpu water depth"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
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
            let view = texture.create_view(&Default::default());
            let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu water depth"),
                layout: &self.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            });
            self.copy = Some(Copy {
                size,
                view,
                bind_group,
            });
        }
        let copy = self.copy.as_ref().expect("made above");
        let samples = target.samples;
        let (source_layout, pipeline) = self.copies.entry(samples).or_insert_with(|| {
            let multisampled = samples > 1;
            let source_layout =
                gpu.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("render-wgpu water depth source"),
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: u32::from(multisampled),
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Depth,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled,
                            },
                            count: None,
                        }],
                    });
            let pipeline_layout =
                gpu.device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("render-wgpu water depth copy"),
                        bind_group_layouts: &[Some(&source_layout)],
                        immediate_size: 0,
                    });
            let pipeline = gpu
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("render-wgpu water depth copy"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &self.shader,
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
                        module: &self.shader,
                        entry_point: Some(if multisampled {
                            "fs_copy_depth_multisampled"
                        } else {
                            "fs_copy_depth"
                        }),
                        compilation_options: Default::default(),
                        targets: &[],
                    }),
                    multiview_mask: None,
                    cache: None,
                });
            (source_layout, pipeline)
        });
        let source = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu water depth source"),
            layout: source_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: u32::from(samples > 1),
                resource: wgpu::BindingResource::TextureView(target.depth),
            }],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render-wgpu water depth copy"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &copy.view,
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
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &source, &[]);
        pass.draw(0..3, 0..1);
    }

    /// The copy's bind group (group 2 of a water pipeline), after `capture`.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.copy.as_ref().expect("captured this view").bind_group
    }
}

/// Group 2 of the water pipelines: the single-sample depth copy.
pub(crate) fn layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
    wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu water depth"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    }
}
