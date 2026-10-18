//! Projected decals (`RenderDiff::SetDecal`, `CameraView.SetDecal`): marks
//! the world pass lays over the opaque surfaces inside a box each
//! (`rusty::decals::decaled`), lit with them. A world-space grid of
//! `CELL_METRES` cells over the ground, hashed into `BUCKETS` buckets, lists
//! the decals whose box reaches each cell, so a fragment tests only its
//! bucket's. The grid and the rows are built on the CPU when the decals
//! change and uploaded in one buffer; a still scene costs no upload.
//! World views lay them; viewmodel and backdrop views bind an empty set.

use std::collections::{BTreeMap, HashMap};

use render_model::{DecalAtlasDescriptor, DecalDescriptor};

use crate::gpu::Gpu;
use crate::tables::GpuTexture;

/// `decals.wgsl` DECAL_BUCKETS, DECAL_CELL_METRES and DECAL_ROW_WORDS.
const BUCKETS: usize = 16384;
const CELL_METRES: f32 = 2.0;
const ROW_WORDS: usize = 28;
const HEADER_WORDS: usize = 4;
const ANGLE_FALLOFF: u32 = 1;
const ON_VEGETATION: u32 = 2;
const ON_ANIMATED: u32 = 4;
/// The sampled textures a world pass binds with decals: the material's 12,
/// the frame's 3 and the view's occlusion (16, WebGPU's guarantee), then the
/// decal atlas and its normals.
pub(crate) const WORLD_SAMPLED_TEXTURES: u32 = 18;

/// What the decals hold and cost, for `engine.renderer`'s `gpu.decals`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DecalReadout {
    /// Why this device lays no decals; None while it can.
    pub refused: Option<String>,
    /// Decals placed.
    pub decals: u32,
    /// Grid entries: a decal counts once in each bucket its box reaches.
    pub entries: u32,
    /// The most decals one bucket lists, which a fragment there tests.
    pub busiest_bucket: u32,
    /// The bytes of the buffer the world pass reads, and of the last upload.
    pub bytes: u64,
    pub upload_bytes: u64,
    /// The CPU microseconds the last rebuild of the grid and rows took.
    pub build_micros: f64,
    /// A normal atlas bends the surfaces.
    pub normals: bool,
}

pub(crate) fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let fragment = |binding, ty| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty,
        count: None,
    };
    let texture = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu decals"),
        entries: &[
            fragment(
                0,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            ),
            fragment(1, texture),
            fragment(
                2,
                wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            ),
            fragment(3, texture),
        ],
    })
}

/// The bucket of the grid cell (x, z) (`decals.wgsl` `decal_bucket`).
fn bucket(cell_x: i32, cell_z: i32) -> usize {
    ((cell_x as u32).wrapping_mul(73_856_093) ^ (cell_z as u32).wrapping_mul(19_349_663)) as usize
        & (BUCKETS - 1)
}

pub(crate) struct Decals {
    decals: BTreeMap<u64, DecalDescriptor>,
    atlas: Option<DecalAtlasDescriptor>,
    /// The words the world pass reads, and the buffer holding them.
    words: Vec<u32>,
    buffer: Option<wgpu::Buffer>,
    world: Option<wgpu::BindGroup>,
    /// A set of no decals, for views that lay none.
    empty: Option<wgpu::BindGroup>,
    /// The decals changed since the last upload; the bind group must be
    /// made again (a new buffer, or the atlas or its textures changed).
    dirty: bool,
    rebind: bool,
    readout: DecalReadout,
}

impl Decals {
    pub fn new(gpu: &Gpu, layout: Option<&wgpu::BindGroupLayout>, white: &GpuTexture) -> Self {
        let empty = layout.map(|layout| {
            let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu no decals"),
                size: (HEADER_WORDS * 4) as u64,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
            bind_group(gpu, layout, &buffer, white, white)
        });
        Self {
            decals: BTreeMap::new(),
            atlas: None,
            words: Vec::new(),
            buffer: None,
            world: None,
            empty,
            dirty: true,
            rebind: true,
            readout: DecalReadout {
                refused: gpu.decals_refusal(),
                ..Default::default()
            },
        }
    }

    pub fn set(&mut self, id: u64, decal: DecalDescriptor) {
        self.decals.insert(id, decal);
        self.dirty = true;
    }

    pub fn remove(&mut self, id: u64) {
        self.dirty |= self.decals.remove(&id).is_some();
    }

    pub fn set_atlas(&mut self, atlas: Option<DecalAtlasDescriptor>) {
        if self.atlas != atlas {
            self.atlas = atlas;
            self.rebind = true;
            self.dirty = true;
        }
    }

    /// Texture `id` was defined or released: the atlas binds it again if it
    /// names it.
    pub fn texture_changed(&mut self, id: &str) {
        if self
            .atlas
            .as_ref()
            .is_some_and(|atlas| atlas.color == id || atlas.normals.as_deref() == Some(id))
        {
            self.rebind = true;
            self.dirty = true;
        }
    }

    /// Upload the grid and rows if the decals changed, and bind them again
    /// if their buffer or textures did. Call before the frame's views.
    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        layout: Option<&wgpu::BindGroupLayout>,
        textures: &HashMap<String, GpuTexture>,
        white: &GpuTexture,
    ) {
        let Some(layout) = layout else {
            return;
        };
        if !self.dirty && !self.rebind {
            return;
        }
        let normals = self
            .atlas
            .as_ref()
            .and_then(|atlas| atlas.normals.as_ref())
            .and_then(|id| textures.get(id));
        if self.dirty {
            let started = std::time::Instant::now();
            self.build(normals.is_some());
            self.readout.build_micros = started.elapsed().as_secs_f64() * 1e6;
            let bytes = (self.words.len() * 4) as u64;
            if self
                .buffer
                .as_ref()
                .is_none_or(|buffer| buffer.size() < bytes)
            {
                self.buffer = Some(gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("render-wgpu decals"),
                    size: bytes.next_power_of_two().max(4096),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.rebind = true;
            }
            let buffer = self.buffer.as_ref().expect("made above");
            gpu.queue
                .write_buffer(buffer, 0, bytemuck::cast_slice(&self.words));
            self.readout.bytes = buffer.size();
            self.readout.upload_bytes = bytes;
            self.dirty = false;
        }
        if self.rebind {
            let color = self
                .atlas
                .as_ref()
                .and_then(|atlas| textures.get(&atlas.color))
                .unwrap_or(white);
            self.world = Some(bind_group(
                gpu,
                layout,
                self.buffer.as_ref().expect("uploaded above"),
                color,
                normals.unwrap_or(white),
            ));
            self.rebind = false;
        }
    }

    /// The set a view's world pass binds as group 3: the decals for world
    /// views, none for the others. None on a device that lays no decals.
    pub fn bind_group(&self, world: bool) -> Option<&wgpu::BindGroup> {
        if world {
            self.world.as_ref().or(self.empty.as_ref())
        } else {
            self.empty.as_ref()
        }
    }

    pub fn readout(&self) -> DecalReadout {
        self.readout.clone()
    }

    /// The header, the buckets' first entries, their lists and the rows.
    fn build(&mut self, normals: bool) {
        let count = self.decals.len();
        // Each decal's buckets: every cell its box's footprint reaches, each
        // bucket once, so no decal lays twice over one fragment.
        let mut placed: Vec<(u32, u32)> = Vec::new();
        let mut buckets: Vec<usize> = Vec::new();
        let mut rows: Vec<u32> = Vec::with_capacity(count * ROW_WORDS);
        for (index, decal) in self.decals.values().enumerate() {
            let (row, low, high) = row(decal);
            rows.extend(row.iter().map(|value| value.to_bits()));
            let cells = |low: f32, high: f32| {
                (low / CELL_METRES).floor() as i32..=(high / CELL_METRES).floor() as i32
            };
            let (xs, zs) = (cells(low[0], high[0]), cells(low[2], high[2]));
            buckets.clear();
            let span = (xs.end() - xs.start() + 1) as usize * (zs.end() - zs.start() + 1) as usize;
            if span >= BUCKETS {
                buckets.extend(0..BUCKETS);
            } else {
                for x in xs {
                    for z in zs.clone() {
                        buckets.push(bucket(x, z));
                    }
                }
                buckets.sort_unstable();
                buckets.dedup();
            }
            placed.extend(buckets.iter().map(|bucket| (*bucket as u32, index as u32)));
        }
        // A counting sort by bucket, keeping each bucket's decals in their
        // placing order.
        let mut first = vec![0u32; BUCKETS + 1];
        for (bucket, _) in &placed {
            first[*bucket as usize + 1] += 1;
        }
        self.readout.busiest_bucket = first.iter().copied().max().unwrap_or(0);
        for bucket in 0..BUCKETS {
            first[bucket + 1] += first[bucket];
        }
        let mut lists = vec![0u32; placed.len()];
        let mut next = first.clone();
        for (bucket, index) in &placed {
            lists[next[*bucket as usize] as usize] = *index;
            next[*bucket as usize] += 1;
        }
        let lists_at = HEADER_WORDS + BUCKETS + 1;
        let rows_at = lists_at + lists.len();
        self.words.clear();
        self.words.extend([
            count as u32,
            rows_at as u32,
            lists_at as u32,
            u32::from(normals),
        ]);
        self.words.extend(first);
        self.words.extend(lists);
        self.words.extend(rows);
        self.readout.decals = count as u32;
        self.readout.entries = placed.len() as u32;
        self.readout.normals = normals;
    }
}

/// A decal's row (`decals.wgsl`), and its box's world bounds.
fn row(decal: &DecalDescriptor) -> ([f32; ROW_WORDS], [f32; 3], [f32; 3]) {
    let rotation = glam::Quat::from_array(decal.rotation).normalize();
    let center = glam::Vec3::from_array(decal.center);
    let size = glam::Vec3::from_array(decal.size).max(glam::Vec3::splat(1e-6));
    let axes = [
        rotation * glam::Vec3::X,
        rotation * glam::Vec3::Y,
        rotation * glam::Vec3::Z,
    ];
    let mut row = [0.0f32; ROW_WORDS];
    for (axis, direction) in axes.iter().enumerate() {
        let to_box = *direction / size[axis];
        row[axis * 4..axis * 4 + 3].copy_from_slice(&to_box.to_array());
        row[axis * 4 + 3] = -to_box.dot(center);
    }
    row[12..16].copy_from_slice(&decal.uv_rect);
    row[16..20].copy_from_slice(&decal.color);
    let flags = if decal.angle_falloff {
        ANGLE_FALLOFF
    } else {
        0
    } | if decal.on_vegetation {
        ON_VEGETATION
    } else {
        0
    } | if decal.on_animated { ON_ANIMATED } else { 0 };
    row[20..24].copy_from_slice(&[
        decal.roughness,
        decal.normal_strength,
        flags as f32,
        decal.fade_start as f32,
    ]);
    row[24..27].copy_from_slice(&axes[1].to_array());
    row[27] = decal.fade_end as f32;
    let reach: glam::Vec3 = axes
        .iter()
        .enumerate()
        .map(|(axis, direction)| direction.abs() * size[axis] * 0.5)
        .sum();
    (
        row,
        (center - reach).to_array(),
        (center + reach).to_array(),
    )
}

fn bind_group(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
    color: &GpuTexture,
    normals: &GpuTexture,
) -> wgpu::BindGroup {
    let (color_view, sampler) = color.material_binding();
    let (normals_view, _) = normals.material_binding();
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("render-wgpu decals"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(color_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(normals_view),
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decal(center: [f32; 3], size: [f32; 3]) -> DecalDescriptor {
        DecalDescriptor {
            center,
            rotation: [0.0, 0.0, 0.0, 1.0],
            size,
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            color: [1.0; 4],
            roughness: 0.0,
            normal_strength: 0.0,
            angle_falloff: false,
            on_vegetation: false,
            on_animated: false,
            fade_start: 0.0,
            fade_end: 0.0,
        }
    }

    /// A decal is listed once in every bucket of the cells its box reaches,
    /// and a fragment's bucket lists every decal over it.
    #[test]
    fn the_grid_lists_each_decal_once_in_every_bucket_it_reaches() {
        let mut decals = Decals {
            decals: BTreeMap::new(),
            atlas: None,
            words: Vec::new(),
            buffer: None,
            world: None,
            empty: None,
            dirty: true,
            rebind: true,
            readout: DecalReadout::default(),
        };
        decals.set(7, decal([0.5, 0.0, 0.5], [0.4, 1.0, 0.4]));
        decals.set(3, decal([3.0, 0.0, -1.0], [6.0, 1.0, 1.0]));
        decals.build(false);
        let words = &decals.words;
        assert_eq!(words[0], 2);
        let (rows, lists) = (words[1] as usize, words[2] as usize);
        let listed = |x: f32, z: f32| -> Vec<u32> {
            let b = bucket(
                (x / CELL_METRES).floor() as i32,
                (z / CELL_METRES).floor() as i32,
            );
            (words[HEADER_WORDS + b]..words[HEADER_WORDS + b + 1])
                .map(|entry| words[lists + entry as usize])
                .collect()
        };
        // Decals are rows in id order: 3 first, then 7.
        assert_eq!(listed(0.5, 0.5), [1]);
        for x in [0.1, 2.5, 5.9] {
            assert!(listed(x, -1.0).contains(&0), "the long decal at x {x}");
        }
        assert_eq!(decals.readout.entries, 1 + 4);
        // The small decal's row maps its centre to the box's middle and its
        // corner to the box's.
        let row = &words[rows + ROW_WORDS..rows + 2 * ROW_WORDS];
        let local = |p: [f32; 3], axis: usize| {
            let r = &row[axis * 4..axis * 4 + 4];
            let r = r
                .iter()
                .map(|word| f32::from_bits(*word))
                .collect::<Vec<_>>();
            r[0] * p[0] + r[1] * p[1] + r[2] * p[2] + r[3]
        };
        assert!(local([0.5, 0.0, 0.5], 0).abs() < 1e-6);
        assert!((local([0.7, 0.0, 0.3], 0) - 0.5).abs() < 1e-5);
        assert!((local([0.7, 0.0, 0.3], 2) + 0.5).abs() < 1e-5);
    }
}
