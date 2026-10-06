//! Chunk distance fields (#5911): the atlas of coarse signed distance
//! fields voxel chunks publish with their meshes, the lookup grid that maps
//! a world cell around the camera to its brick, and the cone-traced
//! occlusion kernel (`distance_field.wgsl`) the ambient occlusion pass runs
//! on its `DistanceField` path in place of a screen-space occlusion pass.
//!
//! A static mesh whose payload carries a field takes a brick when it is
//! defined and frees it when it is released. The atlas is one `r8unorm` 3D
//! texture of bricks; it doubles its bricks per axis when full and uploads
//! every field again from the copies it keeps. The lookup grid is rebuilt
//! on the CPU for each world view from the resident fields' world boxes:
//! `LOOKUP_CELLS`³ cells of one chunk box each, centred on the camera.

use std::collections::HashMap;

use glam::{Mat4, Vec3};

use crate::camera::CameraMatrices;
use crate::frame::PixelRect;
use crate::Gpu;

/// `svc_mesh::distance_field::FIELD_CELLS`: texels per brick axis. A field
/// of another size is not atlased.
pub(crate) const FIELD_CELLS: u32 = 8;
/// `svc_mesh::distance_field::REACH`, in cells.
const REACH_CELLS: f32 = 4.0;
const BRICK_BYTES: usize = (FIELD_CELLS * FIELD_CELLS * FIELD_CELLS) as usize;
const INITIAL_BRICKS_PER_AXIS: u32 = 8;
/// Lookup cells per axis around the camera.
const LOOKUP_CELLS: u32 = 32;
const LOOKUP_ENTRIES: usize = (LOOKUP_CELLS * LOOKUP_CELLS * LOOKUP_CELLS) as usize;
const WORKGROUP: u32 = 16;
/// `FieldParams`: two matrices and four vectors.
const PARAMS_BYTES: u64 = 64 + 64 + 16 + 16 + 16 + 16;
/// Tangent of the cones' half angle.
const CONE_TANGENT: f32 = 0.5;
/// Scales the missing visibility before the product's strength.
const INTENSITY: f32 = 1.2;

/// The distance fields of the last world view, for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistanceFieldReadout {
    /// Why this device cannot cone-trace; `None` while it can.
    pub refused: Option<String>,
    /// Fields resident in the atlas.
    pub resident_fields: u32,
    /// Bricks the atlas holds room for.
    pub atlas_bricks: u32,
    /// Atlas bytes.
    pub atlas_bytes: u64,
    /// Fields placed in the last view's lookup grid.
    pub lookup_entries: u32,
}

/// One resident field in the world: its box and brick.
pub(crate) struct FieldEntry {
    pub min: Vec3,
    pub max: Vec3,
    pub slot: u32,
}

/// The field's box in a mesh's own space, from its payload.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FieldBox {
    pub origin: [f32; 3],
    pub extent: [f32; 3],
}

struct Atlas {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    bricks_per_axis: u32,
    free: Vec<u32>,
    /// Each slot's field, kept to fill a larger atlas.
    fields: HashMap<u32, Vec<u8>>,
}

impl Atlas {
    fn new(device: &wgpu::Device, bricks_per_axis: u32) -> Self {
        let side = bricks_per_axis * FIELD_CELLS;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render-wgpu distance field atlas"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: side,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bricks = bricks_per_axis.pow(3);
        Self {
            texture,
            view,
            bricks_per_axis,
            free: (0..bricks).rev().collect(),
            fields: HashMap::new(),
        }
    }

    fn capacity(&self) -> u32 {
        self.bricks_per_axis.pow(3)
    }

    fn write(&self, queue: &wgpu::Queue, slot: u32, data: &[u8]) {
        let per_axis = self.bricks_per_axis;
        let origin = wgpu::Origin3d {
            x: (slot % per_axis) * FIELD_CELLS,
            y: ((slot / per_axis) % per_axis) * FIELD_CELLS,
            z: (slot / (per_axis * per_axis)) * FIELD_CELLS,
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(FIELD_CELLS),
                rows_per_image: Some(FIELD_CELLS),
            },
            wgpu::Extent3d {
                width: FIELD_CELLS,
                height: FIELD_CELLS,
                depth_or_array_layers: FIELD_CELLS,
            },
        );
    }
}

pub(crate) struct DistanceFields {
    atlas: Atlas,
    /// Counts atlas replacements: a bind group made for an older atlas is
    /// stale.
    generation: u64,
    pipeline: Option<wgpu::ComputePipeline>,
    refused: Option<String>,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params: wgpu::Buffer,
    lookup: wgpu::Buffer,
    lookup_cpu: Vec<u32>,
    last_entries: u32,
}

impl DistanceFields {
    pub fn new(gpu: &Gpu, shader: wgpu::ShaderModule) -> Self {
        let device = &gpu.device;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu distance field occlusion"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let refused = gpu.compute_refusal([WORKGROUP, WORKGROUP, 1], 0);
        let pipeline = refused.is_none().then(|| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu distance field occlusion"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("render-wgpu distance field occlusion"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("cs_field_occlusion"),
                compilation_options: Default::default(),
                cache: None,
            })
        });
        Self {
            atlas: Atlas::new(device, INITIAL_BRICKS_PER_AXIS),
            generation: 0,
            pipeline,
            refused,
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu distance field atlas"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu distance field params"),
                size: PARAMS_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            lookup: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu distance field lookup"),
                size: (LOOKUP_ENTRIES * 4) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            lookup_cpu: vec![0; LOOKUP_ENTRIES],
            last_entries: 0,
        }
    }

    pub fn available(&self) -> bool {
        self.pipeline.is_some()
    }

    /// The atlas a bind group was made for; a different one is stale.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Take a brick for `data` (`FIELD_CELLS`³ bytes), growing the atlas
    /// when it is full. `None` for a field of another size.
    pub fn allocate(&mut self, gpu: &Gpu, data: &[u8]) -> Option<u32> {
        if data.len() != BRICK_BYTES {
            return None;
        }
        if self.atlas.free.is_empty() {
            let mut grown = Atlas::new(&gpu.device, self.atlas.bricks_per_axis * 2);
            let fields = std::mem::take(&mut self.atlas.fields);
            for (slot, field) in &fields {
                grown.write(&gpu.queue, *slot, field);
            }
            grown.free.retain(|slot| !fields.contains_key(slot));
            grown.fields = fields;
            self.atlas = grown;
            self.generation += 1;
        }
        let slot = self.atlas.free.pop()?;
        self.atlas.write(&gpu.queue, slot, data);
        self.atlas.fields.insert(slot, data.to_vec());
        Some(slot)
    }

    pub fn release(&mut self, slot: u32) {
        if self.atlas.fields.remove(&slot).is_some() {
            self.atlas.free.push(slot);
        }
    }

    /// A bind group over a view's depth and occlusion output for the current
    /// atlas.
    pub fn bind_group(
        &self,
        device: &wgpu::Device,
        depth: &wgpu::TextureView,
        occlusion_out: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu distance field occlusion"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(occlusion_out),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.lookup.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&self.atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Place the resident fields around the camera and write the view's
    /// parameters. Every entry's box is one lookup cell; the first entry's
    /// box sets the cell size.
    pub fn begin_view(
        &mut self,
        gpu: &Gpu,
        camera: &CameraMatrices,
        region: PixelRect,
        entries: &[FieldEntry],
    ) {
        let cell_size = entries
            .first()
            .map_or(1.0, |entry| (entry.max - entry.min).max_element().max(1e-3));
        let half = (LOOKUP_CELLS / 2) as f32;
        let origin = (camera.eye / cell_size).floor() * cell_size - Vec3::splat(half * cell_size);
        self.lookup_cpu.fill(0);
        let mut placed = 0;
        for entry in entries {
            let cell = ((entry.min - origin) / cell_size + Vec3::splat(0.5)).floor();
            if cell.min_element() < 0.0 || cell.max_element() >= LOOKUP_CELLS as f32 {
                continue;
            }
            let index = ((cell.z as usize * LOOKUP_CELLS as usize) + cell.y as usize)
                * LOOKUP_CELLS as usize
                + cell.x as usize;
            self.lookup_cpu[index] = entry.slot + 1;
            placed += 1;
        }
        self.last_entries = placed;
        gpu.queue
            .write_buffer(&self.lookup, 0, bytemuck::cast_slice(&self.lookup_cpu));
        let mut params = Vec::with_capacity(PARAMS_BYTES as usize);
        params.extend_from_slice(bytemuck::cast_slice(
            &camera.projection.inverse().to_cols_array(),
        ));
        params.extend_from_slice(bytemuck::cast_slice(&camera.view.inverse().to_cols_array()));
        for value in [region.x, region.y, region.width, region.height] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        for value in [origin.x, origin.y, origin.z, cell_size] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        for value in [LOOKUP_CELLS, self.atlas.bricks_per_axis, FIELD_CELLS, 0] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        let reach = REACH_CELLS * cell_size / FIELD_CELLS as f32;
        for value in [reach, CONE_TANGENT, INTENSITY, 0.0] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        gpu.queue.write_buffer(&self.params, 0, &params);
    }

    /// Dispatch the cone trace over `region` with a bind group from
    /// [`Self::bind_group`].
    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bind_group: &wgpu::BindGroup,
        region: PixelRect,
        timestamp_writes: Option<wgpu::ComputePassTimestampWrites<'_>>,
    ) -> u32 {
        let Some(pipeline) = &self.pipeline else {
            return 0;
        };
        let workgroups = (
            region.width.div_ceil(WORKGROUP),
            region.height.div_ceil(WORKGROUP),
        );
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("ao-occlusion"),
            timestamp_writes,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(workgroups.0, workgroups.1, 1);
        workgroups.0 * workgroups.1
    }

    pub fn readout(&self) -> DistanceFieldReadout {
        let side = u64::from(self.atlas.bricks_per_axis * FIELD_CELLS);
        DistanceFieldReadout {
            refused: self.refused.clone(),
            resident_fields: self.atlas.fields.len() as u32,
            atlas_bricks: self.atlas.capacity(),
            atlas_bytes: side * side * side,
            lookup_entries: self.last_entries,
        }
    }
}

/// A field box in world space under a part's model matrix (chunks only
/// translate, so the box stays a box).
pub(crate) fn world_box(model: &Mat4, field: &FieldBox) -> (Vec3, Vec3) {
    let origin = Vec3::from(field.origin);
    let extent = Vec3::from(field.extent);
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for corner in 0..8 {
        let local = origin
            + Vec3::new(
                if corner & 1 != 0 { extent.x } else { 0.0 },
                if corner & 2 != 0 { extent.y } else { 0.0 },
                if corner & 4 != 0 { extent.z } else { 0.0 },
            );
        let world = model.transform_point3(local);
        min = min.min(world);
        max = max.max(world);
    }
    (min, max)
}
