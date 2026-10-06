//! Density edits: write or blend the per-voxel density that places a
//! reconstructed surface between voxel centres.
//!
//! A density is negative inside, in voxel units (a solid voxel without one
//! reads -0.5, which puts the surface on its cube face). Solidity follows the
//! sign: an edit that makes a voxel's density negative makes it solid with a
//! material, and one that makes it zero or positive empties it. Only the
//! touched chunks' meshes and colliders, and the navigation cells around
//! voxels whose solidity changed, are rebuilt. A failed rebuild restores the
//! scene exactly.

use std::collections::{BTreeMap, BTreeSet};

use core_space::{ChunkCoord, VoxelCoord};
use core_voxel::{VoxelMaterialId, VoxelValue};
use svc_pathfinding::nav_cells_affected_by_voxel;
use svc_volume::{VoxelChunk, DEFAULT_DENSITY_MAGNITUDE};

use crate::voxel_edit::{validate_voxel_address, validate_voxel_material_slot};
use crate::{CollisionSceneError, MaterialVoxel, VoxelCollisionScene, VoxelSourceRevision};

/// Voxels beyond a brush's bounds whose density a union or subtraction still
/// lowers or raises, so the surface near the brush is placed exactly.
const BRUSH_MARGIN_VOXELS: f64 = 2.0;

/// The largest number of voxels one edit batch may visit.
pub const MAX_DENSITY_EDIT_VOXELS: u64 = 16_777_216;

/// A brush in the scene's local frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoxelDensityShape {
    Sphere { center: [f64; 3], radius: f64 },
    Box { min: [f64; 3], max: [f64; 3] },
}

impl VoxelDensityShape {
    /// Signed distance from `point` in world units, negative inside.
    fn distance(self, point: [f64; 3]) -> f64 {
        match self {
            Self::Sphere { center, radius } => {
                let delta: [f64; 3] = std::array::from_fn(|axis| point[axis] - center[axis]);
                (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt() - radius
            }
            Self::Box { min, max } => {
                let q: [f64; 3] = std::array::from_fn(|axis| {
                    let center = (min[axis] + max[axis]) * 0.5;
                    let half = (max[axis] - min[axis]) * 0.5;
                    (point[axis] - center).abs() - half
                });
                let outside = q.map(|value| value.max(0.0));
                (outside[0] * outside[0] + outside[1] * outside[1] + outside[2] * outside[2]).sqrt()
                    + q[0].max(q[1]).max(q[2]).min(0.0)
            }
        }
    }

    fn bounds(self) -> ([f64; 3], [f64; 3]) {
        match self {
            Self::Sphere { center, radius } => {
                (center.map(|c| c - radius), center.map(|c| c + radius))
            }
            Self::Box { min, max } => (min, max),
        }
    }

    fn is_valid(self) -> bool {
        match self {
            Self::Sphere { center, radius } => {
                center.iter().all(|value| value.is_finite()) && radius.is_finite() && radius > 0.0
            }
            Self::Box { min, max } => (0..3).all(|axis| {
                min[axis].is_finite() && max[axis].is_finite() && min[axis] < max[axis]
            }),
        }
    }
}

/// How a brush changes the densities it covers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoxelDensityOperation {
    /// Union: each density becomes the smaller of itself and the brush's
    /// signed distance. Newly solid voxels take the edit's material.
    Add,
    /// Subtraction: each density becomes the larger of itself and the
    /// negated signed distance.
    Subtract,
    /// Move each density inside the brush toward the mean of its six
    /// neighbours by `strength` (0 to 1]. Newly solid voxels take the most
    /// common solid neighbour's material, or the edit's.
    Smooth { strength: f32 },
    /// Give every solid voxel inside the brush the edit's material; densities
    /// are unchanged.
    Paint,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoxelDensityEdit {
    /// Replace the densities of a box of voxels, `size` from `min`, in
    /// x-fastest order. Where a density is negative the voxel is solid with
    /// the matching material, or keeps its own material when that is zero.
    Region {
        min: [i64; 3],
        size: [u32; 3],
        densities: Vec<f32>,
        materials: Vec<u16>,
    },
    Brush {
        shape: VoxelDensityShape,
        operation: VoxelDensityOperation,
        material_slot: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelDensityRejection {
    InvalidRegion {
        edit_index: usize,
    },
    InvalidBrush {
        edit_index: usize,
    },
    InvalidDensity {
        edit_index: usize,
    },
    InvalidMaterialSlot {
        edit_index: usize,
        material_slot: u16,
    },
    /// A region density made an empty voxel solid without naming a material.
    MissingMaterial {
        edit_index: usize,
        address: [i64; 3],
    },
    CoordinateOutOfBounds {
        edit_index: usize,
    },
    TooManyVoxels {
        voxels: u64,
        limit: u64,
    },
    NoChanges,
}

impl std::fmt::Display for VoxelDensityRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelDensityRejection {}

#[derive(Debug)]
pub enum VoxelDensityApplyError {
    Rejected(VoxelDensityRejection),
    /// Rebuilding a touched chunk failed; the edit was reverted.
    ProjectionBuild(CollisionSceneError),
}

impl std::fmt::Display for VoxelDensityApplyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(rejection) => rejection.fmt(formatter),
            Self::ProjectionBuild(error) => write!(formatter, "projection rebuild failed: {error}"),
        }
    }
}

impl std::error::Error for VoxelDensityApplyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelDensityReceipt {
    pub revision_before: VoxelSourceRevision,
    pub accepted_revision: VoxelSourceRevision,
    /// Voxels whose density or material changed.
    pub changed_voxels: usize,
    /// Voxels that became solid or empty.
    pub solidity_changes: usize,
    pub changed_min: [i64; 3],
    pub changed_max_inclusive: [i64; 3],
    pub solid_voxel_count: usize,
    pub authority_hash: u64,
    pub dirty_mesh_chunks: Vec<[i64; 3]>,
    pub rebuilt_mesh_chunks: usize,
    pub reused_mesh_chunks: usize,
    pub removed_mesh_chunks: usize,
    pub mesh_microseconds: u64,
}

/// One voxel as an edit sees it: its material when solid, and its signed
/// density.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sample {
    material: Option<u16>,
    density: f32,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct VoxelDensityEditService;

impl VoxelDensityEditService {
    /// Apply a batch in order; later edits see earlier ones. An invalid edit
    /// rejects the batch before anything is written.
    pub fn apply(
        scene: &mut VoxelCollisionScene,
        edits: &[VoxelDensityEdit],
    ) -> Result<VoxelDensityReceipt, VoxelDensityApplyError> {
        let reject = |rejection| VoxelDensityApplyError::Rejected(rejection);
        let mut visited = 0_u64;
        for (edit_index, edit) in edits.iter().enumerate() {
            visited = visited.saturating_add(validate(scene, edit_index, edit).map_err(reject)?);
        }
        if visited > MAX_DENSITY_EDIT_VOXELS {
            return Err(reject(VoxelDensityRejection::TooManyVoxels {
                voxels: visited,
                limit: MAX_DENSITY_EDIT_VOXELS,
            }));
        }

        // Evaluate every edit against a working copy of the samples it reads.
        let mut working = BTreeMap::<[i64; 3], Sample>::new();
        let read = |scene: &VoxelCollisionScene,
                    working: &BTreeMap<[i64; 3], Sample>,
                    address: [i64; 3]| {
            working
                .get(&address)
                .copied()
                .unwrap_or_else(|| sample(scene, address))
        };
        for (edit_index, edit) in edits.iter().enumerate() {
            match edit {
                VoxelDensityEdit::Region {
                    min,
                    size,
                    densities,
                    materials,
                } => {
                    let mut index = 0;
                    for z in 0..i64::from(size[2]) {
                        for y in 0..i64::from(size[1]) {
                            for x in 0..i64::from(size[0]) {
                                let address = [min[0] + x, min[1] + y, min[2] + z];
                                let density = densities[index];
                                let named = materials.get(index).copied().unwrap_or(0);
                                index += 1;
                                let before = read(scene, &working, address);
                                let material = if density < 0.0 {
                                    match (named, before.material) {
                                        (0, Some(existing)) => Some(existing),
                                        (0, None) => {
                                            return Err(reject(
                                                VoxelDensityRejection::MissingMaterial {
                                                    edit_index,
                                                    address,
                                                },
                                            ))
                                        }
                                        (slot, _) => Some(slot),
                                    }
                                } else {
                                    None
                                };
                                working.insert(address, Sample { material, density });
                            }
                        }
                    }
                }
                VoxelDensityEdit::Brush {
                    shape,
                    operation,
                    material_slot,
                } => {
                    let voxel_size = scene.voxel_size;
                    let margin = match operation {
                        VoxelDensityOperation::Add | VoxelDensityOperation::Subtract => {
                            BRUSH_MARGIN_VOXELS
                        }
                        _ => 0.0,
                    };
                    let (low, high) = brush_voxels(scene, *shape, margin);
                    let mut updates = Vec::new();
                    for z in low[2]..=high[2] {
                        for y in low[1]..=high[1] {
                            for x in low[0]..=high[0] {
                                let address = [x, y, z];
                                let center = voxel_center(scene, address);
                                let distance = (shape.distance(center) / voxel_size) as f32;
                                let before = read(scene, &working, address);
                                let after = match operation {
                                    VoxelDensityOperation::Add => {
                                        let density = before.density.min(distance);
                                        Sample {
                                            density,
                                            material: if density < 0.0 {
                                                before.material.or(Some(*material_slot))
                                            } else {
                                                None
                                            },
                                        }
                                    }
                                    VoxelDensityOperation::Subtract => {
                                        let density = before.density.max(-distance);
                                        Sample {
                                            density,
                                            material: if density < 0.0 {
                                                before.material
                                            } else {
                                                None
                                            },
                                        }
                                    }
                                    VoxelDensityOperation::Smooth { strength } => {
                                        if distance >= 0.0 {
                                            continue;
                                        }
                                        let neighbours = SIX.map(|offset| {
                                            read(scene, &working, add(address, offset))
                                        });
                                        let mean =
                                            neighbours.iter().map(|n| n.density).sum::<f32>() / 6.0;
                                        let density =
                                            before.density + strength * (mean - before.density);
                                        let material = if density < 0.0 {
                                            before
                                                .material
                                                .or_else(|| {
                                                    most_common(
                                                        neighbours
                                                            .iter()
                                                            .filter_map(|n| n.material),
                                                    )
                                                })
                                                .or(Some(*material_slot))
                                        } else {
                                            None
                                        };
                                        Sample { density, material }
                                    }
                                    VoxelDensityOperation::Paint => {
                                        if distance >= 0.0 || before.material.is_none() {
                                            continue;
                                        }
                                        Sample {
                                            material: Some(*material_slot),
                                            ..before
                                        }
                                    }
                                };
                                updates.push((address, after));
                            }
                        }
                    }
                    // Smoothing reads the batch's state before this brush.
                    working.extend(updates);
                }
            }
        }

        let mut changes = Vec::new();
        for (address, after) in working {
            let before = sample(scene, address);
            let density_changed = before.density.abs().to_bits() != after.density.abs().to_bits();
            if before.material != after.material || density_changed {
                changes.push((address, before, after));
            }
        }
        if changes.is_empty() {
            return Err(reject(VoxelDensityRejection::NoChanges));
        }

        let revision_before = scene.source_revision;
        let grid = scene.voxel_world.grid();
        let mut created = Vec::new();
        let mut gained_densities = BTreeSet::new();
        for (address, _, after) in &changes {
            write(scene, *address, *after, &mut created, &mut gained_densities);
        }
        let mut changed_chunks = BTreeSet::new();
        let mut dirty = BTreeSet::new();
        let mut navigation_cells = Vec::new();
        let mut solidity_changes = 0;
        for (address, before, after) in &changes {
            let voxel = VoxelCoord::new(address[0], address[1], address[2]);
            changed_chunks.insert(grid.voxel_to_chunk(voxel));
            dirty.extend(scene.mesh_neighbourhood_of_voxel(voxel));
            if before.material.is_some() != after.material.is_some() {
                solidity_changes += 1;
                navigation_cells
                    .extend(nav_cells_affected_by_voxel(voxel, crate::SCENE_NAVIGATION));
            }
        }
        for coordinate in &created {
            for cell in scene.chunk_cells(*coordinate) {
                navigation_cells.extend(nav_cells_affected_by_voxel(cell, crate::SCENE_NAVIGATION));
            }
        }
        let meshes = match scene.build_meshes(&dirty) {
            Ok(meshes) => meshes,
            Err(error) => {
                for (address, before, _) in changes.iter().rev() {
                    write(
                        scene,
                        *address,
                        *before,
                        &mut Vec::new(),
                        &mut BTreeSet::new(),
                    );
                }
                for coordinate in gained_densities {
                    if let Some(chunk) = scene.voxel_world.get_mut(coordinate) {
                        chunk
                            .set_densities(None)
                            .expect("dropping densities is valid");
                    }
                }
                for coordinate in created {
                    scene.voxel_world.remove(coordinate);
                }
                return Err(VoxelDensityApplyError::ProjectionBuild(error));
            }
        };
        for (address, before, after) in &changes {
            if before.material == after.material {
                continue;
            }
            if let Some(material_slot) = before.material {
                scene.account_voxel(material_voxel(*address, material_slot), false);
            }
            if let Some(material_slot) = after.material {
                scene.account_voxel(material_voxel(*address, material_slot), true);
            }
        }
        scene.publish_local_change(&changed_chunks, &dirty, meshes, navigation_cells);

        let bound = |pick: fn(i64, i64) -> i64| {
            [0, 1, 2].map(|axis| {
                changes
                    .iter()
                    .map(|(address, _, _)| address[axis])
                    .reduce(pick)
                    .expect("at least one change")
            })
        };
        let update = &scene.mesh_update;
        Ok(VoxelDensityReceipt {
            revision_before,
            accepted_revision: scene.source_revision,
            changed_voxels: changes.len(),
            solidity_changes,
            changed_min: bound(i64::min),
            changed_max_inclusive: bound(i64::max),
            solid_voxel_count: scene.solid_voxel_count,
            authority_hash: scene.authority_hash,
            dirty_mesh_chunks: update.dirty_chunks.clone(),
            rebuilt_mesh_chunks: update.rebuilt_chunks,
            reused_mesh_chunks: update.reused_chunks,
            removed_mesh_chunks: update.removed_chunks,
            mesh_microseconds: update.mesh_microseconds,
        })
    }
}

const SIX: [[i64; 3]; 6] = [
    [-1, 0, 0],
    [1, 0, 0],
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
];

fn add(left: [i64; 3], right: [i64; 3]) -> [i64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn most_common(materials: impl Iterator<Item = u16>) -> Option<u16> {
    let mut counts = BTreeMap::<u16, u8>::new();
    for material in materials {
        *counts.entry(material).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|(left, left_count), (right, right_count)| {
            left_count.cmp(right_count).then_with(|| right.cmp(left))
        })
        .map(|(material, _)| material)
}

fn material_voxel(address: [i64; 3], material_slot: u16) -> MaterialVoxel {
    MaterialVoxel {
        state: 0,
        address,
        material_slot,
    }
}

fn voxel_center(scene: &VoxelCollisionScene, address: [i64; 3]) -> [f64; 3] {
    let center = scene
        .voxel_world
        .grid()
        .voxel_center_world(VoxelCoord::new(address[0], address[1], address[2]));
    [center.x, center.y, center.z]
}

/// The inclusive voxel range whose centres lie within the brush's bounds
/// grown by `margin` voxels.
fn brush_voxels(
    scene: &VoxelCollisionScene,
    shape: VoxelDensityShape,
    margin: f64,
) -> ([i64; 3], [i64; 3]) {
    let grid = scene.voxel_world.grid();
    let (low, high) = shape.bounds();
    let grow = margin * scene.voxel_size;
    let first = grid.world_to_voxel(core_space::WorldPos::new(
        low[0] - grow,
        low[1] - grow,
        low[2] - grow,
    ));
    let last = grid.world_to_voxel(core_space::WorldPos::new(
        high[0] + grow,
        high[1] + grow,
        high[2] + grow,
    ));
    (first.to_array(), last.to_array())
}

/// The voxels an edit visits, after checking it.
fn validate(
    scene: &VoxelCollisionScene,
    edit_index: usize,
    edit: &VoxelDensityEdit,
) -> Result<u64, VoxelDensityRejection> {
    let check_material = |material_slot: u16| {
        validate_voxel_material_slot(material_slot).map_err(|_| {
            VoxelDensityRejection::InvalidMaterialSlot {
                edit_index,
                material_slot,
            }
        })
    };
    let check_range = |low: [i64; 3], high: [i64; 3]| {
        if validate_voxel_address(low).is_err() || validate_voxel_address(high).is_err() {
            return Err(VoxelDensityRejection::CoordinateOutOfBounds { edit_index });
        }
        Ok((0..3)
            .map(|axis| (high[axis] - low[axis] + 1).max(0) as u64)
            .product::<u64>())
    };
    match edit {
        VoxelDensityEdit::Region {
            min,
            size,
            densities,
            materials,
        } => {
            let count = size.iter().map(|value| u64::from(*value)).product::<u64>();
            if count == 0
                || densities.len() as u64 != count
                || !(materials.is_empty() || materials.len() as u64 == count)
            {
                return Err(VoxelDensityRejection::InvalidRegion { edit_index });
            }
            if !densities.iter().all(|value| value.is_finite()) {
                return Err(VoxelDensityRejection::InvalidDensity { edit_index });
            }
            for material in materials.iter().filter(|material| **material != 0) {
                check_material(*material)?;
            }
            let high: [i64; 3] = std::array::from_fn(|axis| min[axis] + i64::from(size[axis]) - 1);
            check_range(*min, high)
        }
        VoxelDensityEdit::Brush {
            shape,
            operation,
            material_slot,
        } => {
            if !shape.is_valid() {
                return Err(VoxelDensityRejection::InvalidBrush { edit_index });
            }
            if let VoxelDensityOperation::Smooth { strength } = operation {
                if !(strength.is_finite() && *strength > 0.0 && *strength <= 1.0) {
                    return Err(VoxelDensityRejection::InvalidBrush { edit_index });
                }
            }
            check_material(*material_slot)?;
            let margin = match operation {
                VoxelDensityOperation::Add | VoxelDensityOperation::Subtract => BRUSH_MARGIN_VOXELS,
                _ => 0.0,
            };
            let (low, high) = brush_voxels(scene, *shape, margin);
            check_range(low, high)
        }
    }
}

/// The current material and signed density at `address`. Voxels outside the
/// resident chunks read as empty with the default magnitude.
fn sample(scene: &VoxelCollisionScene, address: [i64; 3]) -> Sample {
    let grid = scene.voxel_world.grid();
    let (coordinate, local) =
        grid.voxel_to_chunk_local(VoxelCoord::new(address[0], address[1], address[2]));
    match scene.voxel_world.get(coordinate) {
        Some(chunk) => Sample {
            material: chunk
                .get(local)
                .and_then(|value| value.material())
                .map(|material| material.raw()),
            density: chunk
                .density(local)
                .expect("local coordinate from the grid"),
        },
        None => Sample {
            material: None,
            density: DEFAULT_DENSITY_MAGNITUDE,
        },
    }
}

fn write(
    scene: &mut VoxelCollisionScene,
    address: [i64; 3],
    sample: Sample,
    created: &mut Vec<ChunkCoord>,
    gained_densities: &mut BTreeSet<ChunkCoord>,
) {
    let grid = scene.voxel_world.grid();
    let (coordinate, local) =
        grid.voxel_to_chunk_local(VoxelCoord::new(address[0], address[1], address[2]));
    if scene.voxel_world.get(coordinate).is_none() {
        if sample.material.is_none() {
            return;
        }
        scene
            .voxel_world
            .insert(coordinate, VoxelChunk::from_spec(&grid));
        created.push(coordinate);
    }
    let chunk = scene
        .voxel_world
        .get_mut(coordinate)
        .expect("resident chunk");
    let value = match sample.material {
        // Density voxels carry no orientation state.
        Some(material_slot) => VoxelValue::solid(VoxelMaterialId::new(material_slot)),
        None => VoxelValue::EMPTY,
    };
    chunk
        .set(local, value)
        .expect("local coordinate from the grid");
    if !chunk.has_densities() {
        if sample.density.abs().to_bits() == DEFAULT_DENSITY_MAGNITUDE.to_bits() {
            return;
        }
        gained_densities.insert(coordinate);
    }
    chunk
        .set_density(local, sample.density)
        .expect("validated finite density");
}
