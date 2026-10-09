//! Scattered grass and clutter on a projected voxel scene (#9546).
//!
//! A product asks for a scatter: which static mesh grows, on which material
//! slots, how densely, how far from the viewer and how varied. The projector
//! places copies on the surface spots `svc_mesh::scatter` finds on each full
//! resolution chunk within reach of the viewer, as one scatter patch node per
//! chunk and scatter, a child of the chunk's node: a world-origin rebase moves
//! the patch with its chunk, and a remesh places it again. Placement is a
//! function of the chunk's mesh and the spot's absolute cell alone, so the same
//! ground grows the same copies every time it comes into reach.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::voxel::{VoxelProjectionError, VoxelRenderKey};
use crate::StableHandleRegistry;
use engine_spatial::{VoxelCollisionScene, VoxelMeshChunk};
use render_model::{
    MeshMaterialSlot, RenderDiff, RenderHandle, ScatterFade, ScatterInstance,
    ScatterPatchDescriptor, ShadowCasting, MAX_SCATTER_PATCH_INSTANCES,
};
use svc_mesh::scatter::{hash, surface_points, unit, ChunkSurface, SurfaceSampling};

/// One kind of thing that grows. Distances are metres in the scene's space.
#[derive(Debug, Clone, PartialEq)]
pub struct VoxelScatter {
    /// The static mesh each copy draws, defined by the caller.
    pub mesh: String,
    pub material_overrides: Vec<MeshMaterialSlot>,
    /// The source material slots it grows on; empty grows on every slot.
    pub slots: BTreeSet<u16>,
    /// Copies per square metre of ground.
    pub density: f32,
    /// Copies stand on chunks within this distance of the viewer, and shrink
    /// away over the last `fade` metres of it.
    pub radius: f32,
    pub fade: f32,
    /// Each copy's scale is drawn between these.
    pub scale: [f32; 2],
    /// Each copy's tint is drawn between these linear colours.
    pub tints: [[f32; 3]; 2],
    /// Ground steeper than this many degrees grows nothing.
    pub slope_limit_degrees: f32,
    /// 0 stands copies upright; 1 stands them along the ground's normal.
    pub align: f32,
    pub shadow_casting: ShadowCasting,
    /// At most this many copies: the nearest chunks' first.
    pub maximum_instances: u32,
    /// Keeps two scatters of the same mesh from growing in the same spots.
    pub seed: u64,
}

/// The most copies per square metre a scatter may ask for.
pub const MAX_SCATTER_DENSITY: f32 = 64.0;

/// A patch is placed once its chunk comes within the radius and kept until
/// the chunk is this many times the radius away, so a viewer on the
/// boundary does not rebuild it.
pub const SCATTER_HYSTERESIS: f64 = 1.1;

#[derive(Debug, Clone, PartialEq)]
pub enum VoxelScatterError {
    Mesh,
    Density,
    Distances,
    Scale,
    Tint,
    Slope,
    Align,
}

impl VoxelScatter {
    pub fn validate(&self) -> Result<(), VoxelScatterError> {
        render_model::validate_asset_id(&self.mesh, render_model::RenderAssetKind::StaticMesh)
            .map_err(|_| VoxelScatterError::Mesh)?;
        if !(self.density > 0.0 && self.density <= MAX_SCATTER_DENSITY) {
            return Err(VoxelScatterError::Density);
        }
        let reach = self.radius.is_finite() && self.radius > 0.0;
        let fade = self.fade.is_finite() && self.fade >= 0.0 && self.fade < self.radius;
        if !(reach && fade) {
            return Err(VoxelScatterError::Distances);
        }
        if !(self
            .scale
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
            && self.scale[0] <= self.scale[1])
        {
            return Err(VoxelScatterError::Scale);
        }
        if !self
            .tints
            .iter()
            .flatten()
            .all(|value| value.is_finite() && (0.0..=16.0).contains(value))
        {
            return Err(VoxelScatterError::Tint);
        }
        if !(0.0..=90.0).contains(&self.slope_limit_degrees) {
            return Err(VoxelScatterError::Slope);
        }
        if !(0.0..=1.0).contains(&self.align) {
            return Err(VoxelScatterError::Align);
        }
        Ok(())
    }

    fn fade_band(&self) -> ScatterFade {
        if self.fade > 0.0 {
            ScatterFade {
                start: self.radius - self.fade,
                end: self.radius,
            }
        } else {
            ScatterFade::default()
        }
    }
}

/// What an instance grows, around where the viewer stands.
#[derive(Debug, Clone, PartialEq)]
pub struct VoxelScatterField {
    /// The viewer, in the instance's scene space.
    pub viewer: [f64; 3],
    pub scatters: Vec<VoxelScatter>,
    /// Boxes none of the scatters grow in.
    pub exclusions: Arc<[VoxelScatterExclusion]>,
}

/// A box no scatter grows in, such as the ground under a built floor: a copy
/// whose base falls inside it is not placed. Its corners are in the frame of
/// a chunk's `origin_voxel` (a scene-space position plus the scene's world
/// origin cell), so a rebase leaves it where it was.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelScatterExclusion {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl VoxelScatterExclusion {
    fn contains(&self, point: [f64; 3]) -> bool {
        (0..3).all(|axis| (self.min[axis]..=self.max[axis]).contains(&point[axis]))
    }

    fn overlaps(&self, (min, max): ([f64; 3], [f64; 3])) -> bool {
        (0..3).all(|axis| self.min[axis] <= max[axis] && min[axis] <= self.max[axis])
    }

    fn key(&self) -> [u64; 6] {
        let [a, b, c] = self.min;
        let [d, e, f] = self.max;
        [a, b, c, d, e, f].map(f64::to_bits)
    }
}

/// The boxes in one set and not the other: where growth changes when the
/// set does.
fn changed_exclusions(
    before: &[VoxelScatterExclusion],
    after: &[VoxelScatterExclusion],
) -> Vec<VoxelScatterExclusion> {
    let keys = |boxes: &[VoxelScatterExclusion]| -> BTreeSet<[u64; 6]> {
        boxes.iter().map(VoxelScatterExclusion::key).collect()
    };
    let (before_keys, after_keys) = (keys(before), keys(after));
    before
        .iter()
        .filter(|exclusion| !after_keys.contains(&exclusion.key()))
        .chain(
            after
                .iter()
                .filter(|exclusion| !before_keys.contains(&exclusion.key())),
        )
        .copied()
        .collect()
}

fn same_exclusions(a: &Arc<[VoxelScatterExclusion]>, b: &Arc<[VoxelScatterExclusion]>) -> bool {
    Arc::ptr_eq(a, b) || a[..] == b[..]
}

/// A chunk's box in the exclusions' frame, a voxel wider each way for
/// surface that strays past it.
fn chunk_bounds(chunk: &VoxelMeshChunk) -> ([f64; 3], [f64; 3]) {
    let size = f64::from(chunk.voxel_size);
    let min = std::array::from_fn(|axis| (chunk.origin_voxel[axis] as f64 - 1.0) * size);
    let max = std::array::from_fn(|axis| {
        (chunk.origin_voxel[axis] as f64 + f64::from(chunk.size[axis]) + 1.0) * size
    });
    (min, max)
}

/// A placed patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PatchSnapshot {
    /// `None` for bare ground: nothing grows there, and it is not placed
    /// again until its chunk remeshes.
    pub handle: Option<RenderHandle>,
    /// The chunk mesh it was placed on.
    pub content_hash: u64,
    pub instances: u32,
}

/// What the renderer holds of an instance's scatters.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScatterSnapshot {
    /// The scatters the patches were placed for.
    pub scatters: Vec<VoxelScatter>,
    /// The exclusions they were placed around.
    pub exclusions: Arc<[VoxelScatterExclusion]>,
    /// By chunk and scatter index.
    pub patches: BTreeMap<([i64; 3], usize), PatchSnapshot>,
    /// Chunks within reach that the instance budget left bare, by scatter,
    /// with the mesh they were counted on and their copies: counted once,
    /// they are not placed again until they remesh.
    pub over_budget: BTreeMap<([i64; 3], usize), (u64, u32)>,
}

impl ScatterSnapshot {
    /// The copies a scatter grows on a chunk's current mesh, when known:
    /// from its patch or its over-budget count.
    fn known_count(&self, chunk: &VoxelMeshChunk, scatter: usize) -> Option<u64> {
        let key = (chunk.chunk, scatter);
        match (self.patches.get(&key), self.over_budget.get(&key)) {
            (Some(patch), _) if patch.content_hash == chunk.content_hash => {
                Some(u64::from(patch.instances))
            }
            (_, Some((hash, count))) if *hash == chunk.content_hash => Some(u64::from(*count)),
            _ => None,
        }
    }

    fn holds(&self, chunk: [i64; 3], scatter: usize) -> bool {
        self.patches.contains_key(&(chunk, scatter))
            || self.over_budget.contains_key(&(chunk, scatter))
    }
}

/// What a scatter's budget gives copies to: the chunks in reach nearest
/// first, each taking its copies while they fit; a chunk that does not fit
/// stays bare and a farther, smaller one may still fit.
struct Selection<'a> {
    grown: Vec<&'a VoxelMeshChunk>,
    bare: Vec<(&'a VoxelMeshChunk, u64)>,
}

/// The selection for one scatter, counting each chunk with `count`; `None`
/// when `count` cannot count a chunk.
fn select<'a>(
    scene: &'a VoxelCollisionScene,
    field: &VoxelScatterField,
    scatter: usize,
    coarse: &BTreeMap<[i64; 3], VoxelMeshChunk>,
    held: &impl Fn(&[i64; 3]) -> bool,
    mut count: impl FnMut(&'a VoxelMeshChunk) -> Option<u64>,
) -> Option<Selection<'a>> {
    let mut budget = u64::from(field.scatters[scatter].maximum_instances);
    let mut selection = Selection {
        grown: Vec::new(),
        bare: Vec::new(),
    };
    for (_, chunk) in reached(scene, field, scatter, coarse, held) {
        let copies = count(chunk)?;
        if copies > budget {
            selection.bare.push((chunk, copies));
        } else {
            budget -= copies;
            selection.grown.push(chunk);
        }
    }
    Some(selection)
}

/// An instance's scatter readout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VoxelScatterReadout {
    pub patches: u64,
    pub instances: u64,
    /// Chunks within reach left bare by a scatter's instance budget.
    pub over_budget: u64,
}

impl ScatterSnapshot {
    pub(crate) fn readout(&self) -> VoxelScatterReadout {
        VoxelScatterReadout {
            patches: self.patches.len() as u64,
            instances: self
                .patches
                .values()
                .map(|patch| u64::from(patch.instances))
                .sum(),
            over_budget: self.over_budget.len() as u64,
        }
    }
}

/// The distance from `viewer` to a chunk's box.
fn chunk_distance(chunk: &VoxelMeshChunk, viewer: [f64; 3]) -> f64 {
    (0..3)
        .map(|axis| {
            let low = f64::from(chunk.translation[axis]);
            let high = low + f64::from(chunk.size[axis]) * f64::from(chunk.voxel_size);
            let outside = (low - viewer[axis]).max(viewer[axis] - high).max(0.0);
            outside * outside
        })
        .sum::<f64>()
        .sqrt()
}

/// The chunks a scatter reaches, nearest first: those within its radius,
/// and placed ones until they are past the hysteresis.
fn reached<'a>(
    scene: &'a VoxelCollisionScene,
    field: &VoxelScatterField,
    scatter: usize,
    coarse: &BTreeMap<[i64; 3], VoxelMeshChunk>,
    placed: &impl Fn(&[i64; 3]) -> bool,
) -> Vec<(f64, &'a VoxelMeshChunk)> {
    let radius = f64::from(field.scatters[scatter].radius);
    let mut chunks: Vec<_> = scene
        .mesh_chunks()
        .filter(|chunk| !coarse.contains_key(&chunk.chunk))
        .map(|chunk| (chunk_distance(chunk, field.viewer), chunk))
        .filter(|(distance, chunk)| {
            *distance <= radius
                || (placed(&chunk.chunk) && *distance <= radius * SCATTER_HYSTERESIS)
        })
        .collect();
    chunks.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.chunk.cmp(&b.1.chunk)));
    chunks
}

/// Whether projecting now would place or remove a patch: the viewer moved a
/// chunk into or out of reach or changed which chunks the budget grows, a
/// chunk in reach has not been counted on its mesh, or the scatters changed.
pub(crate) fn scatter_changed(
    snapshot: &ScatterSnapshot,
    scene: &VoxelCollisionScene,
    field: Option<&VoxelScatterField>,
    coarse: &BTreeMap<[i64; 3], VoxelMeshChunk>,
) -> bool {
    let Some(field) = field else {
        return !snapshot.patches.is_empty();
    };
    if field.scatters != snapshot.scatters
        || !same_exclusions(&field.exclusions, &snapshot.exclusions)
    {
        return true;
    }
    (0..field.scatters.len()).any(|scatter| {
        let held = |coord: &[i64; 3]| snapshot.holds(*coord, scatter);
        let Some(selection) = select(scene, field, scatter, coarse, &held, |chunk| {
            snapshot.known_count(chunk, scatter)
        }) else {
            return true;
        };
        let grown: BTreeSet<[i64; 3]> = selection.grown.iter().map(|chunk| chunk.chunk).collect();
        let bare: BTreeSet<[i64; 3]> = selection
            .bare
            .iter()
            .map(|(chunk, _)| chunk.chunk)
            .collect();
        let had = |keys: &mut dyn Iterator<Item = &([i64; 3], usize)>| -> BTreeSet<[i64; 3]> {
            keys.filter(|(_, index)| *index == scatter)
                .map(|(coord, _)| *coord)
                .collect()
        };
        grown != had(&mut snapshot.patches.keys()) || bare != had(&mut snapshot.over_budget.keys())
    })
}

/// Renderer work for an instance's scatters.
pub(crate) struct ScatterProjection<'a> {
    pub registry: &'a mut StableHandleRegistry<VoxelRenderKey>,
    pub instance_id: &'a str,
    pub snapshot: &'a mut ScatterSnapshot,
    pub operations: &'a mut Vec<RenderDiff>,
}

impl ScatterProjection<'_> {
    /// Bring the patches to the field: remove those whose chunk went, out of
    /// reach or remeshed, and place the ones newly in reach.
    pub(crate) fn project(
        &mut self,
        scene: &VoxelCollisionScene,
        field: Option<&VoxelScatterField>,
        coarse: &BTreeMap<[i64; 3], VoxelMeshChunk>,
    ) -> Result<(), VoxelProjectionError> {
        let scatters = field.map_or(&[][..], |field| &field.scatters[..]);
        let rebuilt = scatters != self.snapshot.scatters;
        if rebuilt {
            self.snapshot.scatters = scatters.to_vec();
            self.snapshot.over_budget.clear();
        }
        let exclusions = field.map_or_else(Default::default, |field| field.exclusions.clone());
        // Chunks a changed exclusion touches are placed again.
        let moved = if rebuilt || same_exclusions(&exclusions, &self.snapshot.exclusions) {
            Vec::new()
        } else {
            changed_exclusions(&self.snapshot.exclusions, &exclusions)
        };
        self.snapshot.exclusions = exclusions.clone();
        let moved_over = |chunk: &VoxelMeshChunk| {
            !moved.is_empty() && {
                let bounds = chunk_bounds(chunk);
                moved.iter().any(|exclusion| exclusion.overlaps(bounds))
            }
        };
        let mut wanted = BTreeMap::<([i64; 3], usize), &VoxelMeshChunk>::new();
        let mut placed = BTreeMap::<([i64; 3], usize), Vec<ScatterInstance>>::new();
        let mut over_budget = BTreeMap::new();
        if let Some(field) = field {
            for (index, scatter) in scatters.iter().enumerate() {
                let snapshot = &*self.snapshot;
                let held = |coord: &[i64; 3]| !rebuilt && snapshot.holds(*coord, index);
                let selection = select(scene, field, index, coarse, &held, |chunk| {
                    Some(
                        match snapshot
                            .known_count(chunk, index)
                            .filter(|_| !rebuilt && !moved_over(chunk))
                        {
                            Some(count) => count,
                            None => {
                                let copies = place(chunk, scatter, &exclusions);
                                let count = copies.len() as u64;
                                placed.insert((chunk.chunk, index), copies);
                                count
                            }
                        },
                    )
                })
                .expect("placing counts every chunk");
                for chunk in selection.grown {
                    wanted.insert((chunk.chunk, index), chunk);
                }
                for (chunk, count) in selection.bare {
                    over_budget.insert(
                        (chunk.chunk, index),
                        (chunk.content_hash, u32::try_from(count).unwrap_or(u32::MAX)),
                    );
                }
            }
        }
        self.snapshot.over_budget = over_budget;
        // Remove the patches no longer wanted as placed.
        let stale: Vec<_> = self
            .snapshot
            .patches
            .iter()
            .filter(|(key, patch)| {
                rebuilt
                    || wanted.get(key).is_none_or(|chunk| {
                        chunk.content_hash != patch.content_hash || moved_over(chunk)
                    })
            })
            .map(|(key, patch)| (*key, patch.handle))
            .collect();
        for ((coord, index), handle) in stale {
            self.snapshot.patches.remove(&(coord, index));
            let Some(handle) = handle else { continue };
            self.registry.remove(&self.key(coord, index));
            // A destroyed chunk took its patches with it.
            if self.chunk(coord).is_some() {
                self.operations.push(RenderDiff::Destroy { handle });
            }
        }
        for ((coord, index), chunk) in wanted {
            if self.snapshot.patches.contains_key(&(coord, index)) {
                continue;
            }
            let Some(parent) = self.chunk(coord) else {
                continue;
            };
            let scatter = &scatters[index];
            let instances = placed
                .remove(&(coord, index))
                .unwrap_or_else(|| place(chunk, scatter, &exclusions));
            if instances.is_empty() {
                self.snapshot.patches.insert(
                    (coord, index),
                    PatchSnapshot {
                        handle: None,
                        content_hash: chunk.content_hash,
                        instances: 0,
                    },
                );
                continue;
            }
            let handle = self
                .registry
                .allocate(self.key(coord, index))
                .map_err(VoxelProjectionError::Handle)?;
            self.snapshot.patches.insert(
                (coord, index),
                PatchSnapshot {
                    handle: Some(handle),
                    content_hash: chunk.content_hash,
                    instances: instances.len() as u32,
                },
            );
            self.operations.push(RenderDiff::CreateScatterPatch {
                handle,
                parent: Some(parent),
                patch: ScatterPatchDescriptor {
                    asset: scatter.mesh.clone(),
                    material_overrides: scatter.material_overrides.clone(),
                    instances,
                    fade: scatter.fade_band(),
                    shadow_casting: scatter.shadow_casting,
                },
            });
        }
        Ok(())
    }

    fn key(&self, chunk: [i64; 3], scatter: usize) -> VoxelRenderKey {
        VoxelRenderKey::Scatter {
            instance: self.instance_id.to_owned(),
            chunk,
            scatter,
        }
    }

    fn chunk(&self, chunk: [i64; 3]) -> Option<RenderHandle> {
        self.registry.handle_of(&VoxelRenderKey::Chunk {
            instance: self.instance_id.to_owned(),
            chunk,
        })
    }
}

#[cfg(test)]
thread_local! {
    /// How many times this thread sampled a chunk for a scatter.
    static PLACEMENTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The copies `scatter` grows on `chunk` outside `exclusions`, in the chunk's
/// space.
pub(crate) fn place(
    chunk: &VoxelMeshChunk,
    scatter: &VoxelScatter,
    exclusions: &[VoxelScatterExclusion],
) -> Vec<ScatterInstance> {
    #[cfg(test)]
    PLACEMENTS.with(|count| count.set(count.get() + 1));
    let groups: Vec<(u16, u32, u32)> = chunk
        .groups
        .iter()
        .filter(|group| scatter.slots.is_empty() || scatter.slots.contains(&group.material_slot))
        .map(|group| (group.material_slot, group.start, group.count))
        .collect();
    if groups.is_empty() {
        return Vec::new();
    }
    let size = f64::from(chunk.voxel_size);
    let origin: [f64; 3] = chunk.origin_voxel.map(|value| value as f64 * size);
    let bounds = chunk_bounds(chunk);
    let exclusions: Vec<_> = exclusions
        .iter()
        .filter(|exclusion| exclusion.overlaps(bounds))
        .collect();
    let points = surface_points(
        &ChunkSurface {
            positions: &chunk.positions,
            indices: &chunk.indices,
            groups: &groups,
            origin,
            band: size,
        },
        &SurfaceSampling {
            density: f64::from(scatter.density),
            seed: scatter.seed,
            minimum_normal_y: f64::from(scatter.slope_limit_degrees).to_radians().cos(),
        },
    );
    points
        .iter()
        .filter(|point| {
            let base = std::array::from_fn(|axis| origin[axis] + f64::from(point.position[axis]));
            !exclusions.iter().any(|exclusion| exclusion.contains(base))
        })
        .take(MAX_SCATTER_PATCH_INSTANCES)
        .map(|point| {
            let draw = |salt: i64| unit(hash(point.key, salt, 0, 0)) as f32;
            let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
            let shade = draw(2);
            let yaw = draw(3) * std::f32::consts::TAU;
            ScatterInstance {
                translation: point.position,
                rotation: multiply(lean(point.normal, scatter.align), about_y(yaw)),
                scale: lerp(scatter.scale[0], scatter.scale[1], draw(1)),
                tint: std::array::from_fn(|channel| {
                    lerp(scatter.tints[0][channel], scatter.tints[1][channel], shade)
                }),
            }
        })
        .collect()
}

fn about_y(angle: f32) -> [f32; 4] {
    let half = angle * 0.5;
    [0.0, half.sin(), 0.0, half.cos()]
}

/// The rotation taking up (+y) `share` of the way to `normal`.
fn lean(normal: [f32; 3], share: f32) -> [f32; 4] {
    // The axis is up × normal; the angle is acos(normal.y).
    let axis = [normal[2], 0.0, -normal[0]];
    let length = (axis[0] * axis[0] + axis[2] * axis[2]).sqrt();
    if share <= 0.0 || length <= 1.0e-6 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let half = normal[1].clamp(-1.0, 1.0).acos() * share * 0.5;
    let scale = half.sin() / length;
    [axis[0] * scale, 0.0, axis[2] * scale, half.cos()]
}

fn multiply(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    let product = [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ];
    let length = product
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    product.map(|value| value / length)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{voxel_material_id, VoxelProjectionInstance, VoxelRenderProjector};
    use engine_spatial::{
        MaterialVoxel, SurfaceMeshOptions, SurfaceMode, VoxelEdit, VoxelEditService, WorldOrigin,
        WorldOriginRebaseRequest, WorldOriginRebaseService, WorldOriginState,
    };
    use render_model::{MaterialUvStrategy, RenderFrameDiff, RenderMaterialDescriptor, Transform};

    fn material(slot: u16) -> RenderMaterialDescriptor {
        RenderMaterialDescriptor {
            texture_transform: None,
            stochastic_tiling: None,
            terrain_layers: None,
            shader: None,
            id: voxel_material_id(slot),
            color: [0.4, 0.5, 0.6, 1.0],
            texture: None,
            roughness: 1.0,
            metalness: 0.0,
            texture_tint: [1.0; 4],
            emission_color: [0.0; 3],
            emission_intensity: 0.0,
            uv_strategy: MaterialUvStrategy::Flat,
            alpha_mode: Default::default(),
            double_sided: false,
            voxel_surface: None,
            normal_map: None,
            triplanar: None,
            emission_map: Default::default(),
            occlusion_map: Default::default(),
            unlit: false,
            flat_shading: false,
            keep_dry: false,
            wind: None,
            water: None,
            translucent_shadow: false,
        }
    }

    /// A dual-contoured floor three voxels thick across four 8³ chunks
    /// along x, its top 3 m up.
    fn floor_scene() -> VoxelCollisionScene {
        let voxels = (0..32).flat_map(|x| {
            (0..8).flat_map(move |z| {
                (0..3).map(move |y| MaterialVoxel {
                    state: 0,
                    address: [x, y, z],
                    material_slot: 1,
                })
            })
        });
        VoxelCollisionScene::from_material_voxels_with_mesh_options(
            1.0,
            8,
            voxels,
            SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring),
        )
        .unwrap()
    }

    fn grass() -> VoxelScatter {
        VoxelScatter {
            mesh: "static-mesh/grass".into(),
            material_overrides: Vec::new(),
            slots: BTreeSet::from([1]),
            density: 2.0,
            radius: 10.0,
            fade: 2.0,
            scale: [0.8, 1.2],
            tints: [[0.8, 0.9, 0.7], [1.0, 1.0, 1.0]],
            slope_limit_degrees: 40.0,
            align: 0.5,
            shadow_casting: ShadowCasting::None,
            maximum_instances: 100_000,
            seed: 11,
        }
    }

    struct Land {
        projector: VoxelRenderProjector,
        exclusions: Arc<[VoxelScatterExclusion]>,
    }

    impl Land {
        fn new() -> Self {
            Self {
                projector: VoxelRenderProjector::new(),
                exclusions: Arc::default(),
            }
        }

        fn project(
            &mut self,
            scene: &VoxelCollisionScene,
            viewer: [f64; 3],
            scatter: VoxelScatter,
        ) -> RenderFrameDiff {
            self.projector.set_scatter(
                "land",
                Some(VoxelScatterField {
                    viewer,
                    scatters: vec![scatter],
                    exclusions: self.exclusions.clone(),
                }),
            );
            self.projector
                .project(
                    &[VoxelProjectionInstance {
                        instance_id: "land".into(),
                        asset_id: "land".into(),
                        transform: Transform::IDENTITY,
                        scene,
                    }],
                    &BTreeMap::from([(1, material(1))]),
                )
                .unwrap()
                .frame
        }

        fn changed(
            &self,
            scene: &VoxelCollisionScene,
            viewer: [f64; 3],
            scatter: VoxelScatter,
        ) -> bool {
            let mut probe = self.projector.clone();
            probe.set_scatter(
                "land",
                Some(VoxelScatterField {
                    viewer,
                    scatters: vec![scatter],
                    exclusions: self.exclusions.clone(),
                }),
            );
            probe.scatter_changed(&[VoxelProjectionInstance {
                instance_id: "land".into(),
                asset_id: "land".into(),
                transform: Transform::IDENTITY,
                scene,
            }])
        }
    }

    /// The patches a frame created, by parent chunk x.
    fn created(land: &Land, frame: &RenderFrameDiff) -> BTreeMap<i64, Vec<ScatterInstance>> {
        frame
            .ops
            .iter()
            .filter_map(|operation| match operation {
                RenderDiff::CreateScatterPatch { parent, patch, .. } => {
                    let x = (0..4)
                        .find(|x| land.projector.chunk_handle("land", [*x, 0, 0]) == *parent)
                        .expect("a patch is a chunk's child");
                    Some((x, patch.instances.clone()))
                }
                _ => None,
            })
            .collect()
    }

    fn destroyed(frame: &RenderFrameDiff) -> usize {
        frame
            .ops
            .iter()
            .filter(|operation| matches!(operation, RenderDiff::Destroy { .. }))
            .count()
    }

    #[test]
    fn chunks_within_reach_grow_copies_on_their_upward_ground() {
        let scene = floor_scene();
        let mut land = Land::new();
        let frame = land.project(&scene, [4.0, 4.0, 4.0], grass());
        let patches = created(&land, &frame);
        assert_eq!(
            patches.keys().copied().collect::<Vec<_>>(),
            [0, 1],
            "chunks 0 and 1 are within 10 m"
        );
        for copies in patches.values() {
            // 64 m² of top at 2 per m²; no copy on the floor's sides or underside.
            assert!(
                (100..=156).contains(&copies.len()),
                "{} copies",
                copies.len()
            );
            for copy in copies {
                assert!((copy.translation[1] - 3.0).abs() < 0.2, "{copy:?}");
                assert!((0.8..=1.2).contains(&copy.scale));
                let [x, y, z, w] = copy.rotation;
                assert!(((x * x + y * y + z * z + w * w).sqrt() - 1.0).abs() < 1e-4);
            }
        }
        let readout = land.projector.scatter_readout("land");
        assert_eq!(readout.patches, 2);
        assert_eq!(
            readout.instances,
            patches
                .values()
                .map(|copies| copies.len() as u64)
                .sum::<u64>()
        );

        // The same ground grows the same copies in a fresh projection.
        let mut again = Land::new();
        let frame = again.project(&scene, [4.0, 4.0, 4.0], grass());
        assert_eq!(created(&again, &frame), patches);

        // Another slot grows nothing.
        let mut elsewhere = Land::new();
        let bare = elsewhere.project(
            &scene,
            [4.0, 4.0, 4.0],
            VoxelScatter {
                slots: BTreeSet::from([2]),
                ..grass()
            },
        );
        assert!(created(&elsewhere, &bare).is_empty());
    }

    #[test]
    fn patches_follow_the_viewer_with_hysteresis_and_the_budget() {
        let scene = floor_scene();
        let mut land = Land::new();
        land.project(&scene, [4.0, 4.0, 4.0], grass());
        // Standing still changes nothing; a step within the hysteresis
        // neither.
        assert!(!land.changed(&scene, [4.0, 4.0, 4.0], grass()));
        assert!(!land.changed(&scene, [3.0, 4.0, 4.0], grass()));
        // Walking along places ahead and removes behind.
        assert!(land.changed(&scene, [28.0, 4.0, 4.0], grass()));
        let frame = land.project(&scene, [28.0, 4.0, 4.0], grass());
        assert_eq!(
            created(&land, &frame).keys().copied().collect::<Vec<_>>(),
            [2, 3]
        );
        assert_eq!(destroyed(&frame), 2);
        assert!(!land.changed(&scene, [28.0, 4.0, 4.0], grass()));

        // A budget for one chunk's copies leaves the farther chunk bare.
        let tight = VoxelScatter {
            maximum_instances: 160,
            ..grass()
        };
        let frame = land.project(&scene, [28.0, 4.0, 4.0], tight.clone());
        assert_eq!(
            created(&land, &frame).keys().copied().collect::<Vec<_>>(),
            [3]
        );
        let readout = land.projector.scatter_readout("land");
        assert_eq!((readout.patches, readout.over_budget), (1, 1));
        assert!(
            !land.changed(&scene, [28.0, 4.0, 4.0], tight),
            "a bare chunk is not placed again"
        );

        // No scatter removes every patch.
        land.projector.set_scatter("land", None);
        let frame = land
            .projector
            .project(
                &[VoxelProjectionInstance {
                    instance_id: "land".into(),
                    asset_id: "land".into(),
                    transform: Transform::IDENTITY,
                    scene: &scene,
                }],
                &BTreeMap::from([(1, material(1))]),
            )
            .unwrap()
            .frame;
        assert_eq!(destroyed(&frame), 1);
        assert_eq!(
            land.projector.scatter_readout("land"),
            VoxelScatterReadout::default()
        );
    }

    /// A budget for one chunk's copies follows the nearest chunk as the
    /// viewer steps across a chunk border, though the chunks in reach stay
    /// the same (#9546 review); a chunk the budget leaves bare is counted
    /// once and not sampled again while its mesh stands.
    #[test]
    fn a_tight_budget_follows_the_nearest_chunk_and_counts_bare_chunks_once() {
        let scene = floor_scene();
        let mut land = Land::new();
        let one_patch = VoxelScatter {
            maximum_instances: 160,
            ..grass()
        };
        let frame = land.project(&scene, [7.0, 4.0, 4.0], one_patch.clone());
        assert_eq!(
            created(&land, &frame).keys().copied().collect::<Vec<_>>(),
            [0]
        );
        assert_eq!(land.projector.scatter_readout("land").over_budget, 2);

        // Standing still samples nothing more.
        let before = PLACEMENTS.with(|count| count.get());
        assert!(!land.changed(&scene, [7.0, 4.0, 4.0], one_patch.clone()));
        land.project(&scene, [7.0, 4.0, 4.0], one_patch.clone());
        assert_eq!(
            PLACEMENTS.with(|count| count.get()),
            before,
            "bare chunks are not sampled again"
        );

        // One step over the border: chunk 1 is now nearest.
        assert!(land.changed(&scene, [9.0, 4.0, 4.0], one_patch.clone()));
        let frame = land.project(&scene, [9.0, 4.0, 4.0], one_patch.clone());
        assert_eq!(
            created(&land, &frame).keys().copied().collect::<Vec<_>>(),
            [1]
        );
        assert_eq!(destroyed(&frame), 1, "chunk 0 gives its copies up");
        let readout = land.projector.scatter_readout("land");
        assert!(
            readout.instances <= 160 && readout.patches == 1,
            "{readout:?}"
        );
        assert!(!land.changed(&scene, [9.0, 4.0, 4.0], one_patch));
    }

    /// Ground that leaves reach and comes back is sampled again, lazily, and
    /// grows exactly the copies it grew before; a remesh samples its chunk
    /// again (the lifecycle the owner accepted for #9546 in place of samples
    /// carried by every resident chunk).
    #[test]
    fn ground_coming_back_into_reach_is_sampled_again_into_the_same_copies() {
        let scene = floor_scene();
        let mut land = Land::new();
        let frame = land.project(&scene, [4.0, 4.0, 4.0], grass());
        let first = created(&land, &frame);
        let away = land.project(&scene, [60.0, 4.0, 4.0], grass());
        assert_eq!(destroyed(&away), first.len());
        let before = PLACEMENTS.with(|count| count.get());
        let back = land.project(&scene, [4.0, 4.0, 4.0], grass());
        assert_eq!(
            PLACEMENTS.with(|count| count.get()),
            before + first.len(),
            "sampled again"
        );
        assert_eq!(created(&land, &back), first, "into the same copies");
    }

    #[test]
    fn an_edit_places_its_chunk_again_and_a_rebase_keeps_every_copy() {
        let mut scene = floor_scene();
        let mut land = Land::new();
        let frame = land.project(&scene, [4.0, 4.0, 4.0], grass());
        let first = created(&land, &frame);
        VoxelEditService::apply(&mut scene, &[VoxelEdit::Clear { address: [2, 2, 2] }]).unwrap();
        let frame = land.project(&scene, [4.0, 4.0, 4.0], grass());
        let replaced = created(&land, &frame);
        assert_eq!(
            replaced.keys().copied().collect::<Vec<_>>(),
            [0],
            "only the edited chunk"
        );
        assert_eq!(destroyed(&frame), 1);
        // Untouched ground keeps its copies; the hole loses some.
        let far_from_hole = |copies: &Vec<ScatterInstance>| -> Vec<ScatterInstance> {
            copies
                .iter()
                .filter(|copy| {
                    (copy.translation[0] - 2.5).abs() > 2.5
                        || (copy.translation[2] - 2.5).abs() > 2.5
                })
                .copied()
                .collect()
        };
        assert_eq!(far_from_hole(&replaced[&0]), far_from_hole(&first[&0]));

        // A rebase moves the chunks, and the patches with them.
        let mut origin = WorldOriginState::default();
        let prepared = WorldOriginRebaseService
            .prepare(
                &origin,
                WorldOriginRebaseRequest {
                    target_origin: WorldOrigin::new([16, 0, 0]),
                    entities: Vec::new(),
                    exclude_outside_envelope: false,
                },
            )
            .unwrap();
        (scene, _) = WorldOriginRebaseService
            .commit(&mut origin, &scene, &prepared)
            .unwrap();
        let frame = land.project(&scene, [4.0 - 16.0, 4.0, 4.0], grass());
        assert!(created(&land, &frame).is_empty(), "{:?}", frame.ops.len());
        assert_eq!(destroyed(&frame), 0);
    }

    /// A box over the floor's top from (x0, z0) to (x1, z1), 2 m to 4 m up.
    fn exclusion(x0: f64, z0: f64, x1: f64, z1: f64) -> VoxelScatterExclusion {
        VoxelScatterExclusion {
            min: [x0, 2.0, z0],
            max: [x1, 4.0, z1],
        }
    }

    #[test]
    fn exclusions_keep_copies_out_and_changing_them_places_only_the_chunks_they_touch() {
        let scene = floor_scene();
        let mut land = Land::new();
        let frame = land_project(&mut land, &scene);
        let open = created(&land, &frame);
        // A floor over chunk 0 from x 2 to 5 and z 1 to 6.
        let floor = exclusion(2.0, 1.0, 5.0, 6.0);
        land.exclusions = Arc::from([floor]);
        assert!(land.changed(&scene, [4.0, 4.0, 4.0], grass()));
        let frame = land_project(&mut land, &scene);
        let built = created(&land, &frame);
        assert_eq!(
            built.keys().copied().collect::<Vec<_>>(),
            [0],
            "only chunk 0"
        );
        assert_eq!(destroyed(&frame), 1);
        let inside = |copy: &ScatterInstance| floor.contains(copy.translation.map(f64::from));
        assert!(open[&0].iter().any(inside), "grass grew there");
        assert!(!built[&0].iter().any(inside), "and grows there no more");
        assert_eq!(
            built[&0],
            open[&0]
                .iter()
                .filter(|copy| !inside(copy))
                .copied()
                .collect::<Vec<_>>(),
            "the rest stays"
        );
        // The same boxes again change nothing.
        land.exclusions = Arc::from([floor]);
        assert!(!land.changed(&scene, [4.0, 4.0, 4.0], grass()));

        // A wall along chunk 1 joins: chunk 0 keeps its patch.
        let wall = exclusion(10.0, 0.0, 10.5, 8.0);
        land.exclusions = Arc::from([floor, wall]);
        let frame = land_project(&mut land, &scene);
        assert_eq!(
            created(&land, &frame).keys().copied().collect::<Vec<_>>(),
            [1]
        );
        assert_eq!(destroyed(&frame), 1);

        // Taking both down grows the open ground again.
        land.exclusions = Arc::default();
        let frame = land_project(&mut land, &scene);
        assert_eq!(created(&land, &frame), open);
    }

    fn land_project(land: &mut Land, scene: &VoxelCollisionScene) -> RenderFrameDiff {
        land.project(scene, [4.0, 4.0, 4.0], grass())
    }
}
