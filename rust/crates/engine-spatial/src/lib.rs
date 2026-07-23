//! Conventional spatial services over the object-centric world kernel.
//!
//! The voxel authority and Parry collision projection are transplanted from
//! Asha unchanged. This crate adapts their typed query vocabulary to a small,
//! centrally scheduled motion system. Gameplay objects remain ordinary entity
//! views with data capabilities; collision internals never become the runtime
//! spine and components do not acquire scattered update hooks.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use core_math::Vec3;
use core_space::{ChunkDims, GridId, VoxelCoord, VoxelGridSpec, WorldPos, WorldVec};
use core_voxel::VoxelValue;
use svc_collision::{CollisionProjection, Ray};
use svc_spatial::VoxelWorld;
use svc_volume::{VolumeError, VoxelChunk};
use world_kernel::{BatchRejection, WorldCommand, WorldCommandBatch, WorldFact, WorldKernel};

/// Upper bound for one scheduled motion phase. The caller controls cadence, but
/// a single accidental multi-second step cannot become an unbounded world edit.
pub const MAX_MOTION_DELTA_SECONDS: f32 = 1.0;
pub const MAX_CHUNK_SIZE: u32 = 64;
pub const MAX_SOLID_VOXELS: usize = 1_000_000;

/// Static collision authority plus its query-optimized derived projection.
///
/// Keeping both layers together preserves the donor's important invariant: the
/// Parry representation accelerates queries but never becomes canonical state.
pub struct VoxelCollisionScene {
    world: VoxelWorld,
    projection: CollisionProjection,
    voxel_size: f64,
    chunk_size: u32,
    solid_voxels: Vec<[i64; 3]>,
}

impl std::fmt::Debug for VoxelCollisionScene {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VoxelCollisionScene")
            .field("voxel_size", &self.voxel_size)
            .field("chunk_size", &self.chunk_size)
            .field("solid_voxel_count", &self.solid_voxels.len())
            .field(
                "resident_chunk_count",
                &self.world.resident_chunks().count(),
            )
            .field("projection_version", &self.projection.version())
            .finish()
    }
}

#[derive(Debug)]
pub enum CollisionSceneError {
    InvalidVoxelSize,
    InvalidChunkSize,
    TooManySolidVoxels {
        limit: usize,
    },
    Volume {
        voxel: [i64; 3],
        source: VolumeError,
    },
}

impl std::fmt::Display for CollisionSceneError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CollisionSceneError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionRayHit {
    pub voxel: [i64; 3],
    pub point: [f64; 3],
    pub distance: f64,
}

impl VoxelCollisionScene {
    /// Build a scene from canonical integer voxel addresses. Input order and
    /// duplicate addresses do not affect the resulting projection.
    pub fn from_solid_voxels(
        voxel_size: f64,
        chunk_size: u32,
        solids: impl IntoIterator<Item = [i64; 3]>,
    ) -> Result<Self, CollisionSceneError> {
        if !(1..=MAX_CHUNK_SIZE).contains(&chunk_size) {
            return Err(CollisionSceneError::InvalidChunkSize);
        }
        let dimensions = ChunkDims::cubic(chunk_size).expect("validated non-zero chunk size");
        let grid = VoxelGridSpec::new(GridId::new(0), voxel_size, dimensions)
            .ok_or(CollisionSceneError::InvalidVoxelSize)?;
        let mut unique_solids = BTreeSet::new();
        for solid in solids {
            unique_solids.insert(solid);
            if unique_solids.len() > MAX_SOLID_VOXELS {
                return Err(CollisionSceneError::TooManySolidVoxels {
                    limit: MAX_SOLID_VOXELS,
                });
            }
        }
        let solid_voxels: Vec<_> = unique_solids.into_iter().collect();
        let mut chunks = BTreeMap::new();

        for &address in &solid_voxels {
            let voxel = VoxelCoord::new(address[0], address[1], address[2]);
            let (chunk_coord, local) = grid.voxel_to_chunk_local(voxel);
            let chunk = chunks
                .entry(chunk_coord)
                .or_insert_with(|| VoxelChunk::from_spec(&grid));
            chunk
                .set(local, VoxelValue::solid_raw(1))
                .map_err(|source| CollisionSceneError::Volume {
                    voxel: address,
                    source,
                })?;
        }

        let mut world = VoxelWorld::new(grid);
        for (coord, chunk) in chunks {
            world.insert(coord, chunk);
        }
        let projection = CollisionProjection::build(&world);
        Ok(Self {
            world,
            projection,
            voxel_size,
            chunk_size,
            solid_voxels,
        })
    }

    pub fn solid_voxel_count(&self) -> usize {
        self.solid_voxels.len()
    }

    pub fn voxel_size(&self) -> f64 {
        self.voxel_size
    }

    pub fn chunk_size(&self) -> u32 {
        self.chunk_size
    }

    pub fn solid_voxels(&self) -> &[[i64; 3]] {
        &self.solid_voxels
    }

    pub fn resident_chunk_count(&self) -> usize {
        self.world.resident_chunks().count()
    }

    pub fn projection_version(&self) -> u64 {
        self.projection.version()
    }

    pub fn contains_point(&self, point: [f64; 3]) -> bool {
        self.projection
            .contains_point(WorldPos::new(point[0], point[1], point[2]))
    }

    pub fn raycast(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
        max_distance: f64,
    ) -> Option<CollisionRayHit> {
        self.projection
            .raycast(
                Ray::new(
                    WorldPos::new(origin[0], origin[1], origin[2]),
                    WorldVec::new(direction[0], direction[1], direction[2]),
                ),
                max_distance,
            )
            .map(|hit| CollisionRayHit {
                voxel: hit.voxel.to_array(),
                point: hit.point.to_array(),
                distance: hit.distance,
            })
    }

    pub fn aabb_overlaps_solid(&self, min: [f64; 3], max: [f64; 3]) -> bool {
        self.projection.aabb_overlaps_solid(
            WorldPos::new(min[0], min[1], min[2]),
            WorldPos::new(max[0], max[1], max[2]),
        )
    }

    fn axis_sweep_overlaps(&self, min: [f64; 3], max: [f64; 3], translation: [f64; 3]) -> bool {
        self.projection.axis_swept_aabb_overlaps_solid(
            WorldPos::new(min[0], min[1], min[2]),
            WorldPos::new(max[0], max[1], max[2]),
            WorldVec::new(translation[0], translation[1], translation[2]),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionAxis {
    X,
    Y,
    Z,
}

impl MotionAxis {
    const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    const fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MotionFact {
    Moved {
        entity: EntityId,
        before: Vec3,
        after: Vec3,
    },
    Blocked {
        entity: EntityId,
        axis: MotionAxis,
        attempted_delta: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MotionPhaseReceipt {
    pub bodies_considered: usize,
    pub moved_bodies: usize,
    pub blocked_axes: usize,
    pub revision_before: u64,
    pub revision_after: u64,
    pub facts: Vec<MotionFact>,
    pub world_facts: Vec<WorldFact>,
}

#[derive(Debug)]
pub enum MotionPhaseError {
    InvalidDeltaSeconds { actual: f32 },
    WorldBatch(BatchRejection),
}

impl std::fmt::Display for MotionPhaseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for MotionPhaseError {}

/// A centrally scheduled service that resolves every kinematic body once.
///
/// Static voxel collision is checked independently on X, Y, then Z. A blocked
/// axis stops and zeroes that velocity component while other axes can still
/// move. All resulting object changes commit as one atomic world batch.
pub struct KinematicMotionSystem;

impl KinematicMotionSystem {
    pub fn run(
        world: &mut WorldKernel,
        scene: &VoxelCollisionScene,
        delta_seconds: f32,
    ) -> Result<MotionPhaseReceipt, MotionPhaseError> {
        if !delta_seconds.is_finite() || !(0.0..=MAX_MOTION_DELTA_SECONDS).contains(&delta_seconds)
        {
            return Err(MotionPhaseError::InvalidDeltaSeconds {
                actual: delta_seconds,
            });
        }

        let bodies: Vec<_> = world.kinematic_bodies().collect();
        let revision_before = world.revision();
        let mut commands = Vec::new();
        let mut facts = Vec::new();
        let mut moved_bodies = 0usize;
        let mut blocked_axes = 0usize;

        for body in &bodies {
            let before = body.translation;
            let mut position = body.translation.to_array();
            let before_velocity = body.velocity;
            let mut velocity = body.velocity.to_array();
            let half_extents = body.half_extents.to_array();

            for axis in MotionAxis::ALL {
                let index = axis.index();
                let delta = velocity[index] * delta_seconds;
                if delta == 0.0 {
                    continue;
                }
                let min = [
                    f64::from(position[0] - half_extents[0]),
                    f64::from(position[1] - half_extents[1]),
                    f64::from(position[2] - half_extents[2]),
                ];
                let max = [
                    f64::from(position[0] + half_extents[0]),
                    f64::from(position[1] + half_extents[1]),
                    f64::from(position[2] + half_extents[2]),
                ];
                let mut translation = [0.0; 3];
                translation[index] = f64::from(delta);

                if scene.axis_sweep_overlaps(min, max, translation) {
                    velocity[index] = 0.0;
                    blocked_axes += 1;
                    facts.push(MotionFact::Blocked {
                        entity: body.entity,
                        axis,
                        attempted_delta: delta,
                    });
                } else {
                    position[index] += delta;
                }
            }

            let after = Vec3::new(position[0], position[1], position[2]);
            let after_velocity = Vec3::new(velocity[0], velocity[1], velocity[2]);
            if after != before {
                moved_bodies += 1;
                commands.push(WorldCommand::SetTranslation {
                    entity: body.entity,
                    translation: after,
                });
                facts.push(MotionFact::Moved {
                    entity: body.entity,
                    before,
                    after,
                });
            }
            if after_velocity != before_velocity {
                commands.push(WorldCommand::SetKinematicVelocity {
                    entity: body.entity,
                    velocity: after_velocity,
                });
            }
        }

        let (revision_after, world_facts) = if commands.is_empty() {
            (revision_before, Vec::new())
        } else {
            let receipt = world
                .apply_batch(WorldCommandBatch::new(commands))
                .map_err(MotionPhaseError::WorldBatch)?;
            (receipt.revision_after, receipt.facts)
        };

        Ok(MotionPhaseReceipt {
            bodies_considered: bodies.len(),
            moved_bodies,
            blocked_axes,
            revision_before,
            revision_after,
            facts,
            world_facts,
        })
    }
}
