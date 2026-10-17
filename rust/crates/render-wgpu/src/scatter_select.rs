//! Per-copy selection of scatter runs (#10017, `scatter_select.wgsl`).
//!
//! With compute and indirect draws, every scatter run a world view may see
//! (a scatter node's copies and its parts by level of detail, `ScatterRun`)
//! is selected copy by copy each view pass: within its patch's fade and past
//! its near cut, kept by its thinning (and scaled up so its ground stays as
//! covered), at the level its distance reaches, crossfading over a level's
//! band, inside the frustum. The parts of every run that share a mesh,
//! material and pipeline draw the copies chosen for them with one indirect
//! draw, so the draws follow the distinct meshes, not the tiles; the world's
//! shadow layers draw a second list that skips the frustum and takes the
//! copies within the sun cascades' reach or inside a fixed layer. The CPU lists then see no scatter run of the
//! world layer, so a run costs the CPU nothing a frame however many copies
//! it holds, and copies past reach, outside the view or thinned away cost
//! the GPU no vertex work. Without compute, runs draw directly at level 0
//! (`Parts::selecting` false) and the readout says why.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};

use glam::{Mat4, Vec3};

use crate::batch::Frustum;
use crate::batch::Pass;
use crate::frame::ViewLayer;
use crate::mesh_pool::MeshPool;
use crate::occlusion::Occluders;
use crate::shaders::Features;
use crate::shadows::CopyReach;
use crate::tables::{GpuMesh, MaterialRef, MeshRef};
use crate::tables::{PartId, Tables};
use crate::timing::{GpuPassTiming, PassTimer};
use crate::Gpu;

const WORKGROUP: u32 = 64;
const PASS: &str = "scatter-select";
/// The fixed shadow layers' spheres `Params` holds; more widen the eye's
/// reach to take them in.
const SPHERES: usize = 16;
/// `Params`: six planes, the eye, the counts, the spheres, the view's
/// matrix and the occluders.
const PARAMS_BYTES: u64 = 6 * 16 + 16 + 16 + SPHERES as u64 * 16 + 64 + 16;
/// `Run`: eight vectors.
const RUN_FLOATS: usize = 32;
/// `wgpu::util::DrawIndexedIndirectArgs`.
pub(crate) const ARGS_BYTES: u64 = 20;
const STATS: usize = 9;
/// Chunks a dispatch row holds (the limit on a dimension).
const ROW_CHUNKS: u32 = 65_535;
/// The most parts an entry can name (20 bits).
const MAX_PART: u32 = 1 << 20;

/// One draw of the selection: the parts sharing a mesh, material and
/// pipeline (`part` one of them, for those), its arguments in the camera's
/// and the shadows' lists, and where its entries start in the selection's
/// region of the instance buffer (in entries, two words each).
#[derive(Clone)]
pub(crate) struct SelectedDraw {
    pub part: PartId,
    pub camera: u32,
    pub shadow: u32,
    /// Its parts' copies may cast.
    pub casts: bool,
    camera_entries: u32,
    shadow_entries: u32,
    /// Its mesh, and its index range there; once pooled
    /// (`MeshPool`), the range in the pool and the mesh's base vertex.
    mesh: MeshRef,
    first_index: u32,
    index_count: u32,
    base_vertex: u32,
    pub pooled: bool,
    /// What draws of one command share: pass, features and material.
    merge: (Pass, Features, (u8, u32)),
    /// Its run: the draws after it in the same run, pooled, draw in one
    /// multi-draw with it.
    pub run: u32,
}

/// The selection of the last world view pass, for diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScatterSelectionReadout {
    /// The last world view selected its scatter runs on the GPU.
    pub enabled: bool,
    /// Why this device cannot; `None` while it can.
    pub refused: Option<String>,
    /// Runs, parts and draws selected, and the copies they hold.
    pub runs: u32,
    pub parts: u32,
    pub draws: u32,
    pub copies: u32,
    /// The commands the camera's draws take (a run sharing pass, features
    /// and material is one multi-draw), and the bytes of the mesh pool they
    /// draw from.
    pub commands: u32,
    pub pool_bytes: u64,
    /// From the last read-back pass: copies tested, drawn for the camera
    /// (a crossfading copy twice), drawn for the shadows, thinned away,
    /// outside the frustum and inside it but hidden behind the view's
    /// opaque depth (`occlusion.rs`), and the camera's draws at levels 1
    /// to 3.
    pub tested: u32,
    pub drawn: u32,
    pub shadow: u32,
    pub thinned: u32,
    pub outside: u32,
    pub occluded: u32,
    pub levels: [u32; 3],
    /// Copies left undrawn because they would pass the device's storage
    /// binding limit: refused their rows, or past the selection's entries.
    pub over_limit: u64,
}

pub(crate) struct ScatterSelect {
    pipeline: Option<wgpu::ComputePipeline>,
    refused: Option<String>,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    runs: wgpu::Buffer,
    chunks: wgpu::Buffer,
    level_parts: wgpu::Buffer,
    pub args: wgpu::Buffer,
    stats: wgpu::Buffer,
    stats_readback: wgpu::Buffer,
    /// Bound in place of a view's occluder pyramid when it has none.
    no_occluders: wgpu::TextureView,
    pending_stats: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
    timer: Option<PassTimer>,
    /// The draws, in order (by features, mesh and material).
    pub draws: Vec<SelectedDraw>,
    part_count: u32,
    chunk_count: u32,
    run_count: u32,
    copy_count: u32,
    /// Entries the region holds: each part's copies, once a list.
    pub entries: u32,
    /// Copies of runs left out because their entries would pass the
    /// binding limit.
    over_budget: u64,
    enabled: bool,
    values: [u32; STATS],
    casts_windy: bool,
    casts_timed: bool,
    commands: u32,
    pool_bytes: u64,
    last_eye: Option<Vec3>,
    eye_moved: bool,
}

impl ScatterSelect {
    pub fn new(gpu: &Gpu, shader: wgpu::ShaderModule) -> Self {
        let device = &gpu.device;
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu scatter select"),
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
                storage(1, true),
                storage(2, true),
                storage(3, true),
                storage(4, true),
                storage(5, true),
                storage(6, false),
                storage(7, false),
                storage(8, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let mut refused = gpu.compute_refusal([WORKGROUP, 1, 1], 0);
        if refused.is_none()
            && !gpu
                .adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION)
        {
            refused = Some("the adapter has no indirect execution".to_owned());
        }
        if refused.is_none()
            && !device
                .features()
                .contains(wgpu::Features::INDIRECT_FIRST_INSTANCE)
        {
            refused = Some("the device has no indirect first instance".to_owned());
        }
        if refused.is_none() && device.limits().max_storage_buffers_per_shader_stage < 8 {
            refused =
                Some("the device binds fewer than 8 storage buffers to a compute stage".to_owned());
        }
        let pipeline = refused.is_none().then(|| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu scatter select"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("render-wgpu scatter select"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("cs_select"),
                compilation_options: Default::default(),
                cache: None,
            })
        });
        let storage_buffer = |label, size: u64, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16),
                usage,
                mapped_at_creation: false,
            })
        };
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        Self {
            pipeline,
            refused,
            layout,
            params: storage_buffer(
                "render-wgpu scatter select params",
                PARAMS_BYTES,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            runs: storage_buffer("render-wgpu scatter select runs", 0, storage),
            chunks: storage_buffer("render-wgpu scatter select chunks", 0, storage),
            level_parts: storage_buffer("render-wgpu scatter select parts", 0, storage),
            args: storage_buffer(
                "render-wgpu scatter select draws",
                0,
                storage | wgpu::BufferUsages::INDIRECT,
            ),
            stats: storage_buffer(
                "render-wgpu scatter select stats",
                (STATS * 4) as u64,
                storage | wgpu::BufferUsages::COPY_SRC,
            ),
            stats_readback: storage_buffer(
                "render-wgpu scatter select stats readback",
                (STATS * 4) as u64,
                wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            ),
            pending_stats: None,
            no_occluders: device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("render-wgpu scatter select no occluders"),
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R32Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default()),
            timer: PassTimer::new(gpu, PASS),
            draws: Vec::new(),
            part_count: 0,
            casts_windy: false,
            casts_timed: false,
            commands: 0,
            pool_bytes: 0,
            last_eye: None,
            eye_moved: false,
            chunk_count: 0,
            run_count: 0,
            copy_count: 0,
            entries: 0,
            over_budget: 0,
            enabled: false,
            values: [0; STATS],
        }
    }

    pub fn available(&self) -> bool {
        self.pipeline.is_some()
    }

    /// Place the draws' meshes in `pool` (each found by `mesh_of`) and
    /// number their runs: a run of draws sharing pass, features and
    /// material, all pooled, is one command. After [`Self::build`].
    pub fn pool_meshes<'a>(
        &mut self,
        gpu: &Gpu,
        pool: &mut MeshPool,
        mesh_of: impl Fn(&MeshRef) -> Option<&'a GpuMesh>,
    ) {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu scatter mesh pool"),
            });
        pool.begin();
        for draw in &mut self.draws {
            let placed = mesh_of(&draw.mesh)
                .and_then(|mesh| pool.place(gpu, &mut encoder, &draw.mesh, mesh));
            draw.pooled = placed.is_some();
            if let Some((base_vertex, first_index)) = placed {
                draw.base_vertex = base_vertex;
                draw.first_index += first_index;
            }
        }
        pool.finish();
        gpu.queue.submit([encoder.finish()]);
        let mut run = 0;
        let mut commands = 0;
        let mut previous: Option<(Pass, Features, (u8, u32))> = None;
        for draw in &mut self.draws {
            if !draw.pooled || previous != Some(draw.merge) {
                run += 1;
                commands += 1;
            }
            previous = draw.pooled.then_some(draw.merge);
            draw.run = run;
        }
        self.commands = commands;
        self.pool_bytes = pool.bytes();
    }

    /// A draw of the selection casts shadows.
    pub fn casts(&self) -> bool {
        self.draws.iter().any(|draw| draw.casts)
    }

    /// A casting draw's shadow changes with presentation time: a product's
    /// caster or displace stage, or the wind while it blows. Its shadows
    /// redraw as time moves.
    pub fn casts_moving(&self, wind_blows: bool) -> bool {
        self.casts_timed || (self.casts_windy && wind_blows)
    }

    /// The last pass selected from another eye than the one before: the
    /// shadows' list (levels, thinning and fades by distance) may differ.
    pub fn eye_moved(&self) -> bool {
        self.eye_moved
    }

    /// Gather the world layer's runs and their parts again: after a regroup,
    /// or a run made, changed or removed. Their entries take at most
    /// `budget`; the runs made last are left out past it.
    pub fn build(&mut self, gpu: &Gpu, tables: &Tables, budget: u64) {
        let parts = &tables.parts;
        let live = |id: PartId| {
            let state = &parts.state[id as usize];
            parts.meta[id as usize].is_some()
                && state.shown
                && ViewLayer::of(state.layer) == ViewLayer::World
                && !state.class.blend
        };
        let mut handles: Vec<_> = tables.scatter_runs.keys().copied().collect();
        handles.sort_unstable();
        let mut runs: Vec<f32> = Vec::new();
        let mut chunks: Vec<u32> = Vec::new();
        // Each level part's id and draw; the draws by what they share.
        let mut level_parts: Vec<[u32; 2]> = Vec::new();
        let mut shared: HashMap<(Features, crate::batch::Pass, u32, bool), u32> = HashMap::new();
        let mut draws: Vec<SelectedDraw> = Vec::new();
        // A draw's parts' runs cast: its shadow list takes entries.
        let mut reserves: Vec<bool> = Vec::new();
        let mut copies = 0u32;
        let mut run_count = 0u32;
        let mut part_count = 0u32;
        let mut used = 0u64;
        let mut over_budget = 0u64;
        for handle in handles {
            let run = &tables.scatter_runs[&handle];
            if !live(run.part) || run.copies.count == 0 {
                continue;
            }
            // Each live level part's copies take at most two entries: the
            // camera's, and the shadows' when its draw holds a casting run.
            let parts_drawn = run
                .levels
                .iter()
                .take(4)
                .flatten()
                .filter(|id| live(**id) && **id < MAX_PART)
                .count() as u64;
            let cost = 2 * parts_drawn * u64::from(run.copies.count);
            if used + cost > budget {
                over_budget += u64::from(run.copies.count);
                continue;
            }
            used += cost;
            let mut level_first = [0u32; 4];
            let mut level_count = [0u32; 4];
            for (level, ids) in run.levels.iter().take(4).enumerate() {
                level_first[level] = level_parts.len() as u32;
                for &id in ids.iter().filter(|id| live(**id) && **id < MAX_PART) {
                    let state = &parts.state[id as usize];
                    let casts = run.casts && state.casts_shadows && state.class.shadow;
                    let draw = *shared
                        .entry((
                            state.class.features,
                            crate::batch::Pass::of(state.class, state.mirrored),
                            state.key,
                            casts,
                        ))
                        .or_insert_with(|| {
                            let meta = parts.meta[id as usize].as_ref().expect("a live part");
                            let material = match &meta.material {
                                MaterialRef::Retained(id) => (0, *id),
                                MaterialRef::Unlit => (1, 0),
                                MaterialRef::LitFallback => (2, 0),
                            };
                            draws.push(SelectedDraw {
                                part: id,
                                camera: 0,
                                shadow: 0,
                                casts,
                                camera_entries: 0,
                                shadow_entries: 0,
                                mesh: meta.mesh.clone(),
                                first_index: meta.first_index,
                                index_count: meta.index_count,
                                base_vertex: 0,
                                pooled: false,
                                merge: (
                                    Pass::of(state.class, state.mirrored),
                                    state.class.features,
                                    material,
                                ),
                                run: 0,
                            });
                            reserves.push(false);
                            draws.len() as u32 - 1
                        });
                    reserves[draw as usize] |= run.casts;
                    // A copy enters a part's list at most once: the draw
                    // holds up to its parts' copies (counted here).
                    draws[draw as usize].camera_entries += run.copies.count;
                    level_parts.push([id, draw]);
                    level_count[level] += 1;
                    part_count += 1;
                }
            }
            let levels = run.levels.len().min(4);
            let mut starts = [0.0f32; 4];
            for (level, start) in run.starts.iter().take(4).enumerate() {
                starts[level] = *start;
            }
            let mut thin_distance = [0.0f32; 4];
            let mut thin_share = [1.0f32; 4];
            for (index, [distance, share]) in run.thinning.iter().take(4).enumerate() {
                thin_distance[index] = *distance;
                thin_share[index] = *share;
            }
            let index = run_count;
            run_count += 1;
            runs.extend_from_slice(&[
                f32::from_bits(run.copies.first),
                f32::from_bits(run.copies.count),
                f32::from_bits(run.part),
                f32::from_bits(levels as u32),
            ]);
            runs.extend_from_slice(&starts);
            runs.extend_from_slice(&[
                run.band,
                run.max_scale.max(1.0),
                run.radius,
                run.thinning.len().min(4) as f32,
            ]);
            runs.extend_from_slice(&thin_distance);
            runs.extend_from_slice(&thin_share);
            runs.extend(level_first.map(f32::from_bits));
            runs.extend(level_count.map(f32::from_bits));
            runs.extend_from_slice(&[
                f32::from_bits(u32::from(run.casts)),
                f32::from_bits(u32::from(run.last_mesh_level)),
                0.0,
                0.0,
            ]);
            for first in (0..run.copies.count).step_by(WORKGROUP as usize) {
                chunks.extend_from_slice(&[index, first]);
            }
            copies += run.copies.count;
        }
        debug_assert_eq!(runs.len(), run_count as usize * RUN_FLOATS);
        // Draw order: by features, then mesh and material, so pipelines and
        // bindings change least; the camera's arguments first, then the
        // shadows', and each draw's entries after the last's.
        let mut order: Vec<u32> = (0..draws.len() as u32).collect();
        // By pass, features and material, so a run of draws sharing them
        // draws as one command from the mesh pool.
        order.sort_by_key(|&draw| {
            let selected = &draws[draw as usize];
            let state = &parts.state[selected.part as usize];
            (selected.merge, state.key, selected.first_index)
        });
        let count = draws.len() as u32;
        let mut entries = 0u32;
        let mut slots = vec![0u32; draws.len()];
        let mut ordered = Vec::with_capacity(draws.len());
        for (slot, &draw) in order.iter().enumerate() {
            let mut selected = draws[draw as usize].clone();
            let capacity = selected.camera_entries;
            selected.camera = slot as u32;
            selected.shadow = count + slot as u32;
            selected.camera_entries = entries;
            selected.shadow_entries = entries + capacity;
            entries += capacity * (1 + u32::from(reserves[draw as usize]));
            debug_assert!(u64::from(entries) <= used);
            slots[draw as usize] = slot as u32;
            ordered.push(selected);
        }
        let level_parts: Vec<u32> = level_parts
            .iter()
            .flat_map(|&[part, draw]| {
                let slot = slots[draw as usize];
                [part, slot, count + slot, 0]
            })
            .collect();
        let upload = |buffer: &mut wgpu::Buffer, label, bytes: &[u8], usage| {
            if bytes.len() as u64 > buffer.size() {
                *buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: (bytes.len() as u64).next_power_of_two(),
                    usage,
                    mapped_at_creation: false,
                });
            }
            if !bytes.is_empty() {
                gpu.queue.write_buffer(buffer, 0, bytes);
            }
        };
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        upload(
            &mut self.runs,
            "render-wgpu scatter select runs",
            bytemuck::cast_slice(&runs),
            storage,
        );
        upload(
            &mut self.chunks,
            "render-wgpu scatter select chunks",
            bytemuck::cast_slice(&chunks),
            storage,
        );
        upload(
            &mut self.level_parts,
            "render-wgpu scatter select parts",
            bytemuck::cast_slice(&level_parts),
            storage,
        );
        let args_bytes = ordered.len() as u64 * 2 * ARGS_BYTES;
        if args_bytes > self.args.size() {
            self.args = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu scatter select draws"),
                size: args_bytes.next_power_of_two(),
                usage: storage | wgpu::BufferUsages::INDIRECT,
                mapped_at_creation: false,
            });
        }
        let caster = |draw: &SelectedDraw| parts.state[draw.part as usize].class.features.caster();
        self.casts_windy = ordered
            .iter()
            .any(|draw| draw.casts && caster(draw).contains(Features::WIND));
        // As the CPU lists' casters: a product caster or displace stage may
        // read presentation time.
        self.casts_timed = ordered.iter().any(|draw| {
            let caster = caster(draw);
            draw.casts
                && (caster.product() != 0 || caster.without(Features::WIND).moves_with_time())
        });
        self.draws = ordered;
        self.part_count = part_count;
        self.chunk_count = (chunks.len() / 2) as u32;
        self.run_count = run_count;
        self.copy_count = copies;
        self.entries = entries;
        self.over_budget = over_budget;
    }

    /// Select the runs for a view pass from `eye` through `view_proj`: its
    /// draws' arguments reset (their entries at `base` in `instances`), the
    /// copies chosen into them (none hidden behind `occluders`, when the
    /// view has them), and the shadows' list of the copies the world's
    /// shadow layers reach (`reaches`, none without shadows).
    #[allow(
        clippy::too_many_arguments,
        reason = "one dispatch over the frame's buffers"
    )]
    pub fn encode(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        buffers: [&wgpu::Buffer; 3],
        base: u32,
        view_proj: &Mat4,
        eye: Vec3,
        reaches: Vec<CopyReach>,
        occluders: Option<Occluders<'_>>,
    ) {
        self.collect(gpu);
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        let Some(pipeline) = &self.pipeline else {
            return;
        };
        self.enabled = true;
        self.eye_moved = self.last_eye != Some(eye);
        self.last_eye = Some(eye);
        if self.draws.is_empty() {
            return;
        }
        let [parts, copies, instances] = buffers;
        // The arguments start empty every pass.
        let mut args = vec![0u8; self.draws.len() * 2 * ARGS_BYTES as usize];
        for draw in &self.draws {
            for (slot, entries) in [
                (draw.camera, draw.camera_entries),
                (draw.shadow, draw.shadow_entries),
            ] {
                let at = slot as usize * ARGS_BYTES as usize;
                args[at..at + ARGS_BYTES as usize].copy_from_slice(
                    wgpu::util::DrawIndexedIndirectArgs {
                        index_count: draw.index_count,
                        instance_count: 0,
                        first_index: draw.first_index,
                        base_vertex: draw.base_vertex as i32,
                        first_instance: base + entries,
                    }
                    .as_bytes(),
                );
            }
        }
        gpu.queue.write_buffer(&self.args, 0, &args);
        let mut params = Vec::with_capacity(PARAMS_BYTES as usize);
        for plane in Frustum::new(view_proj).planes() {
            params.extend_from_slice(bytemuck::cast_slice(&plane.to_array()));
        }
        // The cascades' reach of the eye, and each fixed layer's sphere
        // once, nearest first; past `SPHERES`, the eye's reach grows to
        // take the rest in.
        let mut reach = 0.0f32;
        let mut spheres: Vec<(Vec3, f32)> = Vec::new();
        for layer in reaches {
            match layer {
                CopyReach::Camera(distance) => reach = reach.max(distance),
                CopyReach::Sphere(centre, radius) => {
                    if !spheres.contains(&(centre, radius)) {
                        spheres.push((centre, radius));
                    }
                }
            }
        }
        spheres.sort_by(|a, b| (a.0.distance(eye) - a.1).total_cmp(&(b.0.distance(eye) - b.1)));
        for (centre, radius) in spheres.drain(SPHERES.min(spheres.len())..) {
            reach = reach.max(centre.distance(eye) + radius);
        }
        params.extend_from_slice(bytemuck::cast_slice(&[eye.x, eye.y, eye.z, reach]));
        for value in [self.chunk_count, ROW_CHUNKS, spheres.len() as u32, 0] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        for index in 0..SPHERES {
            let (centre, radius) = spheres.get(index).copied().unwrap_or((Vec3::ZERO, -1.0));
            params.extend_from_slice(bytemuck::cast_slice(&[
                centre.x, centre.y, centre.z, radius,
            ]));
        }
        params.extend_from_slice(bytemuck::cast_slice(&view_proj.to_cols_array()));
        let (pyramid, region, levels) = match &occluders {
            Some(occluders) => (occluders.pyramid, occluders.region, occluders.levels),
            None => (&self.no_occluders, [0, 0], 0),
        };
        for value in [region[0], region[1], levels, 0] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        gpu.queue.write_buffer(&self.params, 0, &params);
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu scatter select"),
            layout: &self.layout,
            entries: &[
                (0, &self.params),
                (1, parts),
                (2, copies),
                (3, &self.runs),
                (4, &self.chunks),
                (5, &self.level_parts),
                (6, &self.args),
                (7, instances),
                (8, &self.stats),
            ]
            .map(|(binding, buffer)| wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            })
            .into_iter()
            .chain([wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::TextureView(pyramid),
            }])
            .collect::<Vec<_>>(),
        });
        encoder.clear_buffer(&self.stats, 0, None);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(PASS),
            timestamp_writes: self.timer.as_ref().and_then(PassTimer::compute_writes),
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let rows = self.chunk_count.div_ceil(ROW_CHUNKS);
        pass.dispatch_workgroups(self.chunk_count.min(ROW_CHUNKS), rows, 1);
        drop(pass);
        if let Some(timer) = &mut self.timer {
            timer.resolve(encoder);
        }
        if self.pending_stats.is_none() {
            encoder.copy_buffer_to_buffer(&self.stats, 0, &self.stats_readback, 0, None);
        }
    }

    /// After the view's encoder was submitted: read the stats and timing.
    pub fn submitted(&mut self) {
        if !self.enabled || self.draws.is_empty() {
            return;
        }
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
        if self.pending_stats.is_none() {
            let (sender, receiver) = std::sync::mpsc::channel();
            self.stats_readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            self.pending_stats = Some(receiver);
        }
    }

    fn collect(&mut self, gpu: &Gpu) {
        let Some(receiver) = self.pending_stats.as_ref() else {
            return;
        };
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        match receiver.try_recv() {
            Err(TryRecvError::Empty) => return,
            Ok(Ok(())) => {
                if let Ok(mapped) = self.stats_readback.slice(..).get_mapped_range() {
                    for (index, value) in self.values.iter_mut().enumerate() {
                        *value = u32::from_le_bytes(
                            mapped[index * 4..index * 4 + 4].try_into().unwrap(),
                        );
                    }
                }
                self.stats_readback.unmap();
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {}
        }
        self.pending_stats = None;
    }

    /// The byte offset of a draw's arguments.
    pub fn args_offset(draw: u32) -> u64 {
        u64::from(draw) * ARGS_BYTES
    }

    pub fn timing(&self) -> GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| crate::timing::untimed(PASS), PassTimer::readout)
    }

    /// The readout, with `refused` copies the copy buffer refused rows.
    pub fn readout(&self, refused: u64) -> ScatterSelectionReadout {
        let [tested, drawn, shadow, thinned, outside, one, two, three, occluded] = self.values;
        ScatterSelectionReadout {
            enabled: self.enabled,
            refused: self.refused.clone(),
            runs: self.run_count,
            parts: self.part_count,
            draws: self.draws.len() as u32,
            commands: self.commands,
            pool_bytes: self.pool_bytes,
            copies: self.copy_count,
            tested,
            drawn,
            shadow,
            thinned,
            outside,
            occluded,
            levels: [one, two, three],
            over_limit: self.over_budget + refused,
        }
    }
}
