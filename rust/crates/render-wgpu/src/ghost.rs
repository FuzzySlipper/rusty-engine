//! Ghost plates: a frozen source appearance shown from 1, 4, 8 or 16
//! captured directions.
//!
//! Each plate keeps an isolated renderer over its `captured_scene` (the
//! Engine's immutable capture-time subtree, lights and pose) and one capture
//! per sector: the source rendered from a perspective camera orbiting its
//! bounds. At draw time the plate re-draws the frozen parts in the plate's
//! placement with a relief warp: each vertex is pulled toward an anchor depth
//! along its capture ray (`depth_retention`) and textured from the sector's
//! capture by its original capture-space position. The plate is unlit and
//! opaque. Exactly one sector draws per view, a hard snap on the viewer's
//! azimuth around the plate with optional hysteresis.
//!
//! Captures run on create and recapture, and on an update that changes the
//! sector count. A placement or relief change only rewrites the sectors'
//! uniforms. Nothing recaptures on its own.

use std::collections::HashMap;
use std::time::Instant;

use glam::{Mat3, Mat4, Quat, Vec3};
use render_host_contracts::RendererCameraProjection;
use render_model::{LightDescriptor, LightShadowIntent, RenderDiff, RenderFrameDiff, RenderHandle};
use render_presentation::{
    GhostPlateAnchorPolicy, GhostPlateCaptureLightingMode, GhostPlateCaptureSettings,
    GhostPlateDescriptor, GhostPlateHandle, GhostPlateMapping, GhostPlateProjectionOp,
    GhostPlateShellMode,
};

use crate::camera::{self, CameraPose};
use crate::capture::{CaptureBackground, CaptureRequest};
use crate::convert;
use crate::pipelines::VERTEX_FLOATS;
use crate::tables::Aabb;
use crate::target::{ColorTarget, DEPTH_FORMAT};
use crate::{Renderer, RendererOptions, ResourceSource};

/// Capture colour: 8-bit sRGB.
const CAPTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// Studio rig light handles inside a plate's isolated renderer.
const STUDIO_HANDLES: [u64; 3] = [u64::MAX - 2, u64::MAX - 1, u64::MAX];
/// `GhostUniform` size (ghost.wgsl): three matrices and two vectors.
const UNIFORM_BYTES: u64 = (16 * 3 + 8) * 4;

/// What the host reads back per plate, for the runtime's ghost plate
/// feedback.
#[derive(Debug, Clone, PartialEq)]
pub struct GhostPlateReadout {
    pub handle: GhostPlateHandle,
    pub source: RenderHandle,
    pub sector_count: u32,
    /// The sector the last view drew.
    pub current_sector: u32,
    /// The last view's azimuth around the plate, in [0, 360).
    pub local_azimuth_degrees: f32,
    pub capture_milliseconds: f64,
    /// Parts of the frozen source the plate redraws.
    pub parts: u32,
    /// Materials the frozen source retains.
    pub materials: u32,
}

struct Sector {
    /// Capture camera to world (source space).
    camera_world: Mat4,
    projection: Mat4,
    /// Source bounds seen from this sector: extents and depth range.
    extent: [f32; 2],
    depth: [f32; 2],
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

pub(crate) struct GhostPlate {
    descriptor: GhostPlateDescriptor,
    /// Renders the frozen capture; its parts are what the plate draws.
    source: Box<Renderer>,
    frame_bind_group: wgpu::BindGroup,
    center: Vec3,
    sectors: Vec<Sector>,
    /// The selected sector per view (keyed by viewport), for hysteresis.
    selection: HashMap<u64, usize>,
    /// The sector to draw in the pass being encoded.
    drawing: usize,
    local_azimuth: f32,
    capture_milliseconds: f64,
}

/// Ghost plate pipelines and layouts, one pipeline per target format.
pub(crate) struct GhostPipelines {
    frame_layout: wgpu::BindGroupLayout,
    sector_layout: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    sampler: wgpu::Sampler,
    by_format: Vec<(ColorTarget, wgpu::RenderPipeline)>,
}

impl GhostPipelines {
    pub fn new(device: &wgpu::Device, shader: wgpu::ShaderModule) -> Self {
        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty,
            count: None,
        };
        let both = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let buffer = |ty| wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu ghost frame"),
            entries: &[
                entry(0, both, buffer(wgpu::BufferBindingType::Uniform)),
                entry(
                    1,
                    both,
                    buffer(wgpu::BufferBindingType::Storage { read_only: true }),
                ),
            ],
        });
        let sector_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu ghost sector"),
            entries: &[
                entry(0, both, buffer(wgpu::BufferBindingType::Uniform)),
                entry(
                    1,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    2,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    3,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu ghost"),
            bind_group_layouts: &[Some(&frame_layout), Some(&sector_layout)],
            immediate_size: 0,
        });
        Self {
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu ghost capture"),
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            }),
            frame_layout,
            sector_layout,
            layout,
            shader,
            by_format: Vec::new(),
        }
    }

    fn ensure(&mut self, device: &wgpu::Device, format: ColorTarget) {
        if self
            .by_format
            .iter()
            .any(|(existing, _)| *existing == format)
        {
            return;
        }
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu ghost"),
            layout: Some(&self.layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_ghost"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: (VERTEX_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            // Double-sided, opaque, depth tested and written.
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: format.multisample(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_ghost"),
                compilation_options: Default::default(),
                targets: &[Some(format.format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.by_format.push((format, pipeline));
    }

    fn get(&self, format: ColorTarget) -> Option<&wgpu::RenderPipeline> {
        self.by_format
            .iter()
            .find(|(existing, _)| *existing == format)
            .map(|(_, pipeline)| pipeline)
    }
}

impl Renderer {
    pub(crate) fn apply_ghost_op(
        &mut self,
        op: &GhostPlateProjectionOp,
        resources: &dyn ResourceSource,
    ) -> Result<(), String> {
        match op {
            GhostPlateProjectionOp::Create { handle, descriptor } => {
                let plate = self.build_ghost_plate(descriptor.clone(), resources)?;
                self.ghosts.insert(*handle, plate);
            }
            GhostPlateProjectionOp::Update { handle, patch } => {
                let plate = self
                    .ghosts
                    .get_mut(handle)
                    .ok_or_else(|| format!("unknown ghost plate {}", handle.raw()))?;
                let mut descriptor = plate.descriptor.clone();
                if let Some(placement) = &patch.placement {
                    descriptor.placement = placement.clone();
                }
                if let Some(config) = &patch.config {
                    descriptor.config = config.clone();
                }
                if descriptor.config.sector_count == plate.descriptor.config.sector_count {
                    plate.descriptor = descriptor;
                    plate.selection.clear();
                    plate.write_uniforms(&self.gpu.queue);
                } else {
                    let plate = self.build_ghost_plate(descriptor, resources)?;
                    self.ghosts.insert(*handle, plate);
                }
            }
            GhostPlateProjectionOp::Recapture {
                handle,
                capture,
                captured_scene,
            } => {
                let plate = self
                    .ghosts
                    .get(handle)
                    .ok_or_else(|| format!("unknown ghost plate {}", handle.raw()))?;
                let mut descriptor = plate.descriptor.clone();
                if let Some(capture) = capture {
                    descriptor.capture = capture.clone();
                }
                if let Some(scene) = captured_scene {
                    descriptor.captured_scene = Some(scene.clone());
                }
                // The replacement is atomic: a failed capture keeps the plate.
                let plate = self.build_ghost_plate(descriptor, resources)?;
                self.ghosts.insert(*handle, plate);
            }
            GhostPlateProjectionOp::Destroy { handle } => {
                self.ghosts
                    .remove(handle)
                    .ok_or_else(|| format!("unknown ghost plate {}", handle.raw()))?;
            }
        }
        Ok(())
    }

    pub fn ghost_plate_readouts(&self) -> Vec<GhostPlateReadout> {
        let mut readouts: Vec<GhostPlateReadout> = self
            .ghosts
            .iter()
            .map(|(handle, plate)| GhostPlateReadout {
                handle: *handle,
                source: plate.descriptor.source,
                sector_count: plate.sectors.len() as u32,
                current_sector: plate.drawing as u32,
                local_azimuth_degrees: plate.local_azimuth,
                capture_milliseconds: plate.capture_milliseconds,
                parts: plate.source.table_counts().parts as u32,
                materials: plate.source.table_counts().materials as u32,
            })
            .collect();
        readouts.sort_by_key(|readout| readout.handle);
        readouts
    }

    /// Capture every sector of a plate from its frozen scene.
    fn build_ghost_plate(
        &self,
        descriptor: GhostPlateDescriptor,
        resources: &dyn ResourceSource,
    ) -> Result<GhostPlate, String> {
        let started = Instant::now();
        let frame = descriptor
            .captured_scene
            .clone()
            .ok_or("ghost plate has no captured scene")?;
        let settings = &descriptor.capture;
        let isolated_lighting = settings.lighting.mode == GhostPlateCaptureLightingMode::Isolated;
        let mut source = Box::new(Renderer::new(
            &self.gpu,
            RendererOptions {
                default_world_lights: self.options.default_world_lights && !isolated_lighting,
                default_viewmodel_lights: false,
                shadows: false,
                shadow_budget: None,
                // A capture is isolated geometry: no screen-space occlusion.
                ambient_occlusion: Default::default(),
                clustered_lighting: false,
                gpu_culling: false,
                samples: 1,
                vsync: false,
                render_scale: 1.0,
                volumetric_fog: render_model::VolumetricFogQuality::Off,
                volumetric_clouds: render_model::VolumetricCloudsQuality::Off,
            },
        ));
        source.set_animation_time(self.animation_time);
        let mut issues = source.apply(&frame, resources);
        // The source appears even if the product hides it (CraftSurvive
        // publishes its source invisible).
        let mut ops = vec![RenderDiff::Update {
            handle: descriptor.source,
            transform: None,
            material: None,
            visible: Some(true),
            metadata: None,
        }];
        if isolated_lighting {
            // The studio rig replaces the scene's lights.
            let lights: Vec<RenderHandle> = source.tables.lights.iter().copied().collect();
            ops.extend(
                lights
                    .into_iter()
                    .map(|handle| RenderDiff::Destroy { handle }),
            );
            ops.extend(
                studio_rig(settings, Quat::IDENTITY)
                    .into_iter()
                    .zip(STUDIO_HANDLES)
                    .map(|(light, handle)| RenderDiff::CreateLight {
                        handle: RenderHandle::new(handle),
                        parent: None,
                        light,
                    }),
            );
        }
        issues.extend(source.apply(&unpublished(ops), resources));
        if let Some(issue) = issues.first() {
            return Err(format!(
                "ghost plate capture: the frozen scene's {} op is not realized: {}",
                issue.op, issue.detail
            ));
        }
        source.prepare();
        let bounds = source.subtree_bounds(descriptor.source);
        if bounds.is_empty() {
            return Err("ghost plate capture: the source draws nothing".to_owned());
        }
        let center = (bounds.min + bounds.max) * 0.5;
        let size = bounds.max - bounds.min;
        let radius = (size.length() * 1.7).max(1.0);
        let count = usize::from(descriptor.config.sector_count.max(1));
        let resolution = u32::from(settings.resolution);
        let projection = RendererCameraProjection::Perspective {
            fov_y_degrees: f64::from(settings.field_of_view_degrees),
            near: f64::from(settings.near),
            far: f64::from(settings.far),
        };
        let mut sectors = Vec::with_capacity(count);
        for sector in 0..count {
            let azimuth =
                normalized_azimuth(settings.azimuth_degrees + sector as f32 * 360.0 / count as f32)
                    .to_radians();
            let elevation = settings.elevation_degrees.to_radians();
            let position = center
                + Vec3::new(
                    azimuth.sin() * elevation.cos(),
                    elevation.sin(),
                    azimuth.cos() * elevation.cos(),
                ) * radius;
            let pose = look_at(position, center);
            if isolated_lighting {
                let ops = studio_rig(settings, pose.orientation)
                    .into_iter()
                    .zip(STUDIO_HANDLES)
                    .map(|(light, handle)| RenderDiff::UpdateLight {
                        handle: RenderHandle::new(handle),
                        light,
                    })
                    .collect();
                source.apply(&unpublished(ops), resources);
            }
            let capture = source.capture(&CaptureRequest {
                pose,
                projection,
                width: resolution,
                height: resolution,
                format: CAPTURE_FORMAT,
                background: CaptureBackground::Clear([0.0; 4]),
                viewmodel: false,
            });
            let matrices = camera::camera_matrices(pose, &projection, 1.0);
            let camera_world = matrices.view.inverse();
            let (extent, depth) = projected_bounds(&bounds, &matrices.view);
            let uniform = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu ghost sector"),
                size: UNIFORM_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let color = capture.color.create_view(&Default::default());
            let depth_view = capture.depth.create_view(&Default::default());
            let bind_group = self
                .gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("render-wgpu ghost sector"),
                    layout: &self.ghost_pipelines.sector_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniform.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&color),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&depth_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&self.ghost_pipelines.sampler),
                        },
                    ],
                });
            sectors.push(Sector {
                camera_world,
                projection: matrices.projection,
                extent,
                depth,
                uniform,
                bind_group,
            });
        }
        let frame_bind_group = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu ghost frame"),
                layout: &self.ghost_pipelines.frame_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.frame_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: source.parts_buffer.as_entire_binding(),
                    },
                ],
            });
        let plate = GhostPlate {
            descriptor,
            source,
            frame_bind_group,
            center,
            sectors,
            selection: HashMap::new(),
            drawing: 0,
            local_azimuth: 0.0,
            capture_milliseconds: started.elapsed().as_secs_f64() * 1000.0,
        };
        plate.write_uniforms(&self.gpu.queue);
        Ok(plate)
    }

    /// Exact world bounds of a node's subtree at its current pose.
    fn subtree_bounds(&self, root: RenderHandle) -> Aabb {
        let mut nodes = vec![root];
        let mut bounds = Aabb::EMPTY;
        while let Some(handle) = nodes.pop() {
            let Some(node) = self.tables.nodes.get(&handle) else {
                continue;
            };
            nodes.extend(node.children.iter().copied());
            for part in &node.parts {
                let Some(meta) = self.tables.parts.meta[*part as usize].as_ref() else {
                    continue;
                };
                let Some(mesh) = self.mesh(&meta.mesh) else {
                    continue;
                };
                let world = Mat4::from_cols_slice(
                    &self.tables.parts.gpu[*part as usize * crate::tables::PART_ROW_FLOATS..][..16],
                );
                let first = meta.first_index as usize;
                let last = (first + meta.index_count as usize).min(mesh.cpu.indices.len());
                for index in &mesh.cpu.indices[first..last] {
                    bounds.include(world.transform_point3(mesh.cpu.positions[*index as usize]));
                }
            }
        }
        bounds
    }

    /// Choose each plate's sector for the view about to be encoded.
    pub(crate) fn select_ghost_sectors(&mut self, eye: Vec3, view: u64, format: ColorTarget) {
        if self.ghosts.is_empty() {
            return;
        }
        self.ghost_pipelines.ensure(&self.gpu.device, format);
        for plate in self.ghosts.values_mut() {
            let placement = &plate.descriptor.placement.transform;
            let rotation = convert::rotation(placement.rotation);
            let relative = rotation.inverse() * (eye - convert::vec3(placement.translation));
            if relative.length_squared() < 1e-10 {
                // The viewer is at the plate: keep the current sector.
                plate.drawing = plate.selection.get(&view).copied().unwrap_or(0);
                continue;
            }
            let local = normalize_degrees(relative.x.atan2(relative.z).to_degrees());
            let current = plate.selection.get(&view).copied().unwrap_or(0);
            let selected = select_sector(
                local,
                plate.descriptor.capture.azimuth_degrees,
                plate.sectors.len(),
                current,
                plate.descriptor.config.sector_hysteresis_degrees,
            );
            plate.selection.insert(view, selected);
            plate.drawing = selected;
            plate.local_azimuth = local;
        }
    }

    /// Draw every plate's selected sector into the world pass.
    pub(crate) fn draw_ghost_plates(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
    ) -> u32 {
        let Some(pipeline) = self.ghost_pipelines.get(format) else {
            return 0;
        };
        let mut draws = 0;
        for plate in self.ghosts.values() {
            let Some(sector) = plate.sectors.get(plate.drawing) else {
                continue;
            };
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &plate.frame_bind_group, &[]);
            pass.set_bind_group(1, &sector.bind_group, &[]);
            let source = &plate.source;
            for (id, part) in source.tables.parts.meta.iter().enumerate() {
                let Some(part) = part else { continue };
                let state = &source.tables.parts.state[id];
                if !state.shown || state.class.lines {
                    continue;
                }
                let Some(mesh) = source.mesh(&part.mesh) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(
                    part.first_index..part.first_index + part.index_count,
                    0,
                    id as u32..id as u32 + 1,
                );
                draws += 1;
            }
        }
        draws
    }
}

impl GhostPlate {
    /// Per-sector display transforms and relief parameters.
    fn write_uniforms(&self, queue: &wgpu::Queue) {
        let placement = &self.descriptor.placement;
        let config = &self.descriptor.config;
        let capture = &self.descriptor.capture;
        let plate_world = convert::transform_matrix(&placement.transform);
        let (near, far) = (capture.near, capture.far);
        // The shell tolerance is widened by half of one 8-bit step of linear
        // depth over [near, far].
        let half_step = (far - near) / 510.0;
        for sector in &self.sectors {
            let scale = (placement.width / sector.extent[0].max(1e-6))
                .min(placement.height / sector.extent[1].max(1e-6));
            let source_to_display =
                Mat4::from_scale(Vec3::splat(scale)) * Mat4::from_translation(-self.center);
            let display = plate_world * source_to_display * sector.camera_world;
            let anchor = match config.anchor_policy {
                GhostPlateAnchorPolicy::BoundsCenter => (sector.depth[0] + sector.depth[1]) * 0.5,
                GhostPlateAnchorPolicy::BoundsNormalized => {
                    sector.depth[0] + (sector.depth[1] - sector.depth[0]) * config.anchor_value
                }
            };
            let mut floats: Vec<f32> = Vec::with_capacity((UNIFORM_BYTES / 4) as usize);
            floats.extend_from_slice(&display.to_cols_array());
            floats.extend_from_slice(&sector.camera_world.inverse().to_cols_array());
            floats.extend_from_slice(&sector.projection.to_cols_array());
            floats.extend_from_slice(&[anchor, config.depth_retention, near, far]);
            floats.extend_from_slice(&[
                config.shell_depth_epsilon + half_step,
                match config.shell_mode {
                    GhostPlateShellMode::WholeMesh => 0.0,
                    GhostPlateShellMode::StrictSource => 1.0,
                    GhostPlateShellMode::RepairedSource => 2.0,
                },
                match config.plate_mapping {
                    GhostPlateMapping::PlateLocked => 0.0,
                    GhostPlateMapping::ProjectiveSurface => 1.0,
                },
                1.0 / f32::from(capture.resolution),
            ]);
            queue.write_buffer(&sector.uniform, 0, bytemuck::cast_slice(&floats));
        }
    }
}

fn unpublished(ops: Vec<RenderDiff>) -> RenderFrameDiff {
    RenderFrameDiff {
        publication: None,
        ops,
    }
}

/// The isolated-lighting studio rig for one sector: an ambient light and key
/// and fill directional lights whose "toward light" directions are in the
/// capture camera's frame (`orientation`).
fn studio_rig(settings: &GhostPlateCaptureSettings, orientation: Quat) -> [LightDescriptor; 3] {
    let lighting = &settings.lighting;
    let travel =
        |toward: [f32; 3]| convert::array(-(orientation * convert::vec3(toward).normalize()));
    [
        LightDescriptor::Ambient {
            color: lighting.ambient_color,
            intensity: lighting.ambient_intensity,
            enabled: true,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
            range: None,
        },
        LightDescriptor::Directional {
            color: lighting.key_color,
            intensity: lighting.key_intensity,
            enabled: true,
            direction: travel(lighting.key_direction),
            range: None,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
        LightDescriptor::Directional {
            color: lighting.fill_color,
            intensity: lighting.fill_intensity,
            enabled: true,
            direction: travel(lighting.fill_direction),
            range: None,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    ]
}

/// A camera at `eye` looking at `target` with +Y up (+Z when vertical).
fn look_at(eye: Vec3, target: Vec3) -> CameraPose {
    let forward = (target - eye).normalize_or(Vec3::NEG_Z);
    let up = if forward.y.abs() > 0.999 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let right = forward.cross(up).normalize();
    let up = right.cross(forward);
    CameraPose {
        position: eye,
        orientation: Quat::from_mat3(&Mat3::from_cols(right, up, -forward)),
    }
}

/// The bounds' capture-view extents (x, y) and depth range (−z).
fn projected_bounds(bounds: &Aabb, view: &Mat4) -> ([f32; 2], [f32; 2]) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for corner in 0..8 {
        let point = Vec3::new(
            if corner & 1 == 0 {
                bounds.min.x
            } else {
                bounds.max.x
            },
            if corner & 2 == 0 {
                bounds.min.y
            } else {
                bounds.max.y
            },
            if corner & 4 == 0 {
                bounds.min.z
            } else {
                bounds.max.z
            },
        );
        let local = view.transform_point3(point);
        min = min.min(local);
        max = max.max(local);
    }
    ([max.x - min.x, max.y - min.y], [-max.z, -min.z])
}

/// Wrap to (−180, 180].
fn normalized_azimuth(degrees: f32) -> f32 {
    let wrapped = (degrees + 180.0).rem_euclid(360.0) - 180.0;
    if wrapped == -180.0 {
        180.0
    } else {
        wrapped
    }
}

fn normalize_degrees(degrees: f32) -> f32 {
    degrees.rem_euclid(360.0)
}

fn signed_angular_difference(value: f32, reference: f32) -> f32 {
    (value - reference + 540.0).rem_euclid(360.0) - 180.0
}

/// Keep the current sector within half a sector plus the hysteresis,
/// otherwise snap to the nearest.
fn select_sector(local: f32, base: f32, count: usize, current: usize, hysteresis: f32) -> usize {
    if count <= 1 {
        return 0;
    }
    let width = 360.0 / count as f32;
    let current_center = base + current as f32 * width;
    if signed_angular_difference(local, current_center).abs() <= width * 0.5 + hysteresis {
        return current;
    }
    let nearest = (signed_angular_difference(local, base) / width).round() as i64;
    nearest.rem_euclid(count as i64) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_selection_keeps_the_current_sector_within_hysteresis() {
        // (local, base, count, current, hysteresis).
        assert_eq!(select_sector(24.0, 0.0, 8, 0, 3.0), 0);
        assert_eq!(select_sector(26.0, 0.0, 8, 0, 3.0), 1);
        assert_eq!(select_sector(339.0, 0.0, 8, 0, 3.0), 0);
        assert_eq!(select_sector(334.0, 0.0, 8, 0, 3.0), 7);
        assert_eq!(select_sector(190.0, 10.0, 4, 0, 3.0), 2);
        assert_eq!(select_sector(270.0, 0.0, 1, 0, 22.5), 0);
    }

    #[test]
    fn azimuths_wrap_into_the_half_open_turn() {
        assert_eq!(normalized_azimuth(180.0), 180.0);
        assert_eq!(normalized_azimuth(-180.0), 180.0);
        assert_eq!(normalized_azimuth(270.0), -90.0);
        assert_eq!(normalize_degrees(-10.0), 350.0);
    }
}
