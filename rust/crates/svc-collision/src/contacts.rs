//! Contact readouts for a query shape against the projected world and
//! caller-posed targets: every target a shape overlaps
//! ([`CollisionProjection::overlap_contacts`]), and every target a shape
//! moving from one pose to another meets, by time of impact
//! ([`CollisionProjection::sweep_contacts`]).
//!
//! Targets are call-local: oriented boxes, capsules, spheres and segments,
//! each naming an entity, a part of it and the entity holding it. The world
//! answers as one voxel contact (the deepest overlap, or the earliest
//! impact, across chunks) and one contact per static-mesh instance. Every
//! target answers at most once. Nothing is retained; the order is the
//! contacts' fraction, then source (voxels, static meshes by instance, then
//! entities by entity and part).

use parry3d_f64::math::{Pose, Rotation, Vector};
use parry3d_f64::query::{
    cast_shapes, cast_shapes_nonlinear, contact, intersection_test, NonlinearRigidMotion,
    ShapeCastHit, ShapeCastOptions, ShapeCastStatus,
};
use parry3d_f64::shape::{Ball, Capsule, Cuboid, Segment, Shape};

use crate::{character_query_may_intersect, CollisionProjection, StaticMeshAssetId};
use crate::{StaticMeshInstanceId, WorldAabb};
use core_space::WorldPos;

/// A shape in its own frame. A pose places the frame; a sweep turns it about
/// a pivot given in the same frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContactShape {
    Box {
        center: [f64; 3],
        half_extents: [f64; 3],
    },
    Capsule {
        a: [f64; 3],
        b: [f64; 3],
        radius: f64,
    },
    Sphere {
        center: [f64; 3],
        radius: f64,
    },
    Segment {
        a: [f64; 3],
        b: [f64; 3],
    },
}

/// Where a shape's frame stands: a translation and a rotation `[x, y, z, w]`
/// (normalized on use).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactPose {
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
}

impl ContactPose {
    pub const fn at(translation: [f64; 3]) -> Self {
        Self {
            translation,
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

/// One caller-posed target: an entity, the part of it the shape is, and the
/// entity holding it (zero when none).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactTarget {
    pub entity: u64,
    pub part: u32,
    pub holder: u64,
    pub shape: ContactShape,
    pub pose: ContactPose,
}

/// What a contact touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContactSource {
    Voxel,
    StaticMesh {
        instance: StaticMeshInstanceId,
        asset: StaticMeshAssetId,
        geometry_hash: u64,
    },
    Entity {
        entity: u64,
        part: u32,
        holder: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeContact {
    pub source: ContactSource,
    /// Where along a sweep the contact begins, 0 to 1; 0 for an overlap.
    pub fraction: f64,
    /// On the touched surface, in world space.
    pub point: [f64; 3],
    /// The touched surface's outward normal, in world space; zero when an
    /// overlap is too deep to measure one.
    pub normal: [f64; 3],
    /// How far the shapes overlap, for an overlap or a sweep that starts
    /// inside its target.
    pub penetration_depth: f64,
    /// The sweep started inside this target.
    pub start_solid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactQueryError {
    InvalidShape,
    InvalidPose,
    UnsupportedBackendQuery,
}

impl std::fmt::Display for ContactQueryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "contact query rejected: {self:?}")
    }
}

impl std::error::Error for ContactQueryError {}

/// Rotations closer than this (radians) to none sweep in a straight line.
const STILL_ROTATION: f64 = 1e-9;

fn vector([x, y, z]: [f64; 3]) -> Vector {
    Vector::new(x, y, z)
}

fn array(value: Vector) -> [f64; 3] {
    [value.x, value.y, value.z]
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

/// A shape built for parry, and the offset of its parry frame in its own
/// frame (parry boxes and balls are centred on their frame).
struct Built {
    shape: Box<dyn Shape>,
    offset: Vector,
}

fn build(shape: ContactShape) -> Result<Built, ContactQueryError> {
    let built = match shape {
        ContactShape::Box {
            center,
            half_extents,
        } => {
            if !finite(&center) || !finite(&half_extents) || half_extents.iter().any(|h| *h < 0.0) {
                return Err(ContactQueryError::InvalidShape);
            }
            Built {
                shape: Box::new(Cuboid::new(vector(half_extents))),
                offset: vector(center),
            }
        }
        ContactShape::Capsule { a, b, radius } => {
            if !finite(&a) || !finite(&b) || !radius.is_finite() || radius <= 0.0 {
                return Err(ContactQueryError::InvalidShape);
            }
            Built {
                shape: Box::new(Capsule::new(vector(a), vector(b), radius)),
                offset: Vector::ZERO,
            }
        }
        ContactShape::Sphere { center, radius } => {
            if !finite(&center) || !radius.is_finite() || radius <= 0.0 {
                return Err(ContactQueryError::InvalidShape);
            }
            Built {
                shape: Box::new(Ball::new(radius)),
                offset: vector(center),
            }
        }
        ContactShape::Segment { a, b } => {
            if !finite(&a) || !finite(&b) {
                return Err(ContactQueryError::InvalidShape);
            }
            Built {
                shape: Box::new(Segment::new(vector(a), vector(b))),
                offset: Vector::ZERO,
            }
        }
    };
    Ok(built)
}

fn pose(value: ContactPose) -> Result<Pose, ContactQueryError> {
    let [x, y, z, w] = value.rotation;
    let length = (x * x + y * y + z * z + w * w).sqrt();
    if !finite(&value.translation) || !finite(&value.rotation) || length < 1e-12 {
        return Err(ContactQueryError::InvalidPose);
    }
    Ok(Pose::from_parts(
        vector(value.translation),
        Rotation::from_xyzw(x / length, y / length, z / length, w / length),
    ))
}

/// A built target placed in the world.
struct Placed {
    source: ContactSource,
    built: Built,
    pose: Pose,
}

fn place(targets: &[ContactTarget]) -> Result<Vec<Placed>, ContactQueryError> {
    targets
        .iter()
        .map(|target| {
            let built = build(target.shape)?;
            let pose = pose(target.pose)? * Pose::from_translation(built.offset);
            Ok(Placed {
                source: ContactSource::Entity {
                    entity: target.entity,
                    part: target.part,
                    holder: target.holder,
                },
                built,
                pose,
            })
        })
        .collect()
}

fn bounds_of(min: Vector, max: Vector) -> Option<WorldAabb> {
    finite(&[min.x, min.y, min.z, max.x, max.y, max.z]).then(|| WorldAabb {
        min: WorldPos::new(min.x, min.y, min.z),
        max: WorldPos::new(max.x, max.y, max.z),
    })
}

fn unsupported<T>(_: T) -> ContactQueryError {
    ContactQueryError::UnsupportedBackendQuery
}

fn sort(contacts: &mut [ShapeContact]) {
    contacts.sort_by(|left, right| {
        left.fraction
            .total_cmp(&right.fraction)
            .then(left.source.cmp(&right.source))
    });
}

/// The world's shapes a query within `bounds` can touch, each with the
/// source it answers as.
fn world_shapes(
    projection: &CollisionProjection,
    bounds: Option<WorldAabb>,
) -> Vec<(ContactSource, &dyn Shape)> {
    let mut shapes = Vec::new();
    for (_, collider) in projection.character_candidates(bounds) {
        if !character_query_may_intersect(bounds, collider.bounds) {
            continue;
        }
        for shape in collider.shapes() {
            shapes.push((ContactSource::Voxel, shape));
        }
    }
    for (instance, asset, geometry_hash, shape, shape_bounds) in
        projection.static_meshes.character_shapes()
    {
        if character_query_may_intersect(bounds, shape_bounds) {
            shapes.push((
                ContactSource::StaticMesh {
                    instance,
                    asset,
                    geometry_hash,
                },
                shape.as_ref(),
            ));
        }
    }
    shapes
}

/// Keeps one contact per source: the earliest, then the deepest.
fn keep(contacts: &mut Vec<ShapeContact>, contact: ShapeContact) {
    match contacts
        .iter_mut()
        .find(|known| known.source == contact.source)
    {
        Some(known) => {
            let earlier = contact.fraction < known.fraction;
            let deeper = contact.fraction == known.fraction
                && contact.penetration_depth > known.penetration_depth;
            if earlier || deeper {
                *known = contact;
            }
        }
        None => contacts.push(contact),
    }
}

/// The overlap of `shape` at `query` with one target shape, when they share
/// any volume (touching surfaces do not overlap).
fn overlap(
    source: ContactSource,
    query: &Pose,
    shape: &dyn Shape,
    target: &Pose,
    target_shape: &dyn Shape,
) -> Result<Option<ShapeContact>, ContactQueryError> {
    let measured = contact(query, shape, target, target_shape, 0.0).map_err(unsupported)?;
    let contact = match measured {
        Some(found) if found.dist < 0.0 => ShapeContact {
            source,
            fraction: 0.0,
            point: array(found.point2),
            normal: array(found.normal2),
            penetration_depth: -found.dist,
            start_solid: false,
        },
        // Deep inside a composite (a closed mesh) parry measures no contact
        // at all; the shapes still overlap. A contact at a distance of zero
        // or more is touching, not overlapping.
        None if intersection_test(query, shape, target, target_shape).map_err(unsupported)? => {
            ShapeContact {
                source,
                fraction: 0.0,
                point: array(query.translation),
                normal: [0.0; 3],
                penetration_depth: 0.0,
                start_solid: false,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(contact))
}

impl CollisionProjection {
    /// Every target, and the world, that `shape` placed at `at` overlaps.
    /// `world` false leaves the world out.
    pub fn overlap_contacts(
        &self,
        shape: ContactShape,
        at: ContactPose,
        targets: &[ContactTarget],
        world: bool,
    ) -> Result<Vec<ShapeContact>, ContactQueryError> {
        let built = build(shape)?;
        let query = pose(at)? * Pose::from_translation(built.offset);
        let placed = place(targets)?;
        let aabb = built.shape.compute_aabb(&query);
        let bounds = bounds_of(aabb.mins, aabb.maxs);
        let mut contacts = Vec::new();
        if world {
            for (source, shape) in world_shapes(self, bounds) {
                if let Some(found) =
                    overlap(source, &query, built.shape.as_ref(), &Pose::IDENTITY, shape)?
                {
                    keep(&mut contacts, found);
                }
            }
        }
        for target in &placed {
            if let Some(found) = overlap(
                target.source,
                &query,
                built.shape.as_ref(),
                &target.pose,
                target.built.shape.as_ref(),
            )? {
                keep(&mut contacts, found);
            }
        }
        sort(&mut contacts);
        Ok(contacts)
    }

    /// Every target, and the world, that `shape` meets moving from `start`
    /// to `end`: translating its `pivot` (in the shape's frame) in a straight
    /// line while turning about it by the shortest rotation between the two
    /// poses. Each answers with its first time of impact, so a target lying
    /// wholly between the end poses is met. A target the shape starts inside
    /// answers at fraction 0 with `start_solid` and its overlap.
    pub fn sweep_contacts(
        &self,
        shape: ContactShape,
        start: ContactPose,
        end: ContactPose,
        pivot: [f64; 3],
        targets: &[ContactTarget],
        world: bool,
    ) -> Result<Vec<ShapeContact>, ContactQueryError> {
        if !finite(&pivot) {
            return Err(ContactQueryError::InvalidShape);
        }
        let built = build(shape)?;
        let (from, to) = (pose(start)?, pose(end)?);
        let placed = place(targets)?;
        let pivot = vector(pivot);
        let mut turn = to.rotation * from.rotation.inverse();
        if turn.w < 0.0 {
            turn = -turn;
        }
        let angular = turn.to_scaled_axis();
        let linear = to.transform_point(pivot) - from.transform_point(pivot);
        let initial = from * Pose::from_translation(built.offset);
        let motion = NonlinearRigidMotion::new(initial, pivot - built.offset, linear, angular);
        let straight = angular.length() < STILL_ROTATION;
        let bounds = sweep_bounds(&built, from, pivot, linear);

        let cast = |target: &Pose, target_shape: &dyn Shape| -> Result<Option<ShapeCastHit>, _> {
            if straight {
                cast_shapes(
                    &initial,
                    linear,
                    built.shape.as_ref(),
                    target,
                    Vector::ZERO,
                    target_shape,
                    ShapeCastOptions {
                        max_time_of_impact: 1.0,
                        target_distance: 0.0,
                        stop_at_penetration: true,
                        compute_impact_geometry_on_penetration: true,
                    },
                )
            } else {
                cast_shapes_nonlinear(
                    &motion,
                    built.shape.as_ref(),
                    &NonlinearRigidMotion::constant_position(*target),
                    target_shape,
                    0.0,
                    1.0,
                    true,
                )
            }
            .map_err(unsupported)
        };
        let mut contacts = Vec::new();
        let mut meet = |source: ContactSource,
                        target: &Pose,
                        target_shape: &dyn Shape|
         -> Result<(), ContactQueryError> {
            let Some(hit) = cast(target, target_shape)? else {
                return Ok(());
            };
            let start_solid = hit.status == ShapeCastStatus::PenetratingOrWithinTargetDist;
            let found = if start_solid {
                // The overlap the sweep starts in, measured where it starts.
                overlap(source, &initial, built.shape.as_ref(), target, target_shape)?.map(
                    |found| ShapeContact {
                        start_solid: true,
                        ..found
                    },
                )
            } else {
                None
            };
            let contact = found.unwrap_or_else(|| ShapeContact {
                source,
                fraction: hit.time_of_impact.clamp(0.0, 1.0),
                point: array(target.transform_point(hit.witness2)),
                normal: array(target.rotation * hit.normal2),
                penetration_depth: 0.0,
                start_solid,
            });
            keep(&mut contacts, contact);
            Ok(())
        };
        if world {
            for (source, shape) in world_shapes(self, bounds) {
                meet(source, &Pose::IDENTITY, shape)?;
            }
        }
        for target in &placed {
            meet(target.source, &target.pose, target.built.shape.as_ref())?;
        }
        sort(&mut contacts);
        Ok(contacts)
    }
}

/// A conservative bound of the swept shape: the pivot's path, widened by the
/// farthest any point of the shape lies from the pivot.
fn sweep_bounds(built: &Built, from: Pose, pivot: Vector, linear: Vector) -> Option<WorldAabb> {
    let local = built
        .shape
        .compute_aabb(&Pose::from_translation(built.offset));
    let reach = (0..8)
        .map(|corner| {
            let pick = |bit: usize, low: f64, high: f64| if corner & bit == 0 { low } else { high };
            Vector::new(
                pick(1, local.mins.x, local.maxs.x),
                pick(2, local.mins.y, local.maxs.y),
                pick(4, local.mins.z, local.maxs.z),
            )
            .distance(pivot)
        })
        .fold(0.0, f64::max);
    let start = from.transform_point(pivot);
    let end = start + linear;
    let reach = Vector::splat(reach);
    bounds_of(start.min(end) - reach, start.max(end) + reach)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
    use core_voxel::VoxelValue;
    use svc_spatial::VoxelWorld;
    use svc_volume::VoxelChunk;

    fn spec() -> VoxelGridSpec {
        VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(8).unwrap()).unwrap()
    }

    fn empty() -> CollisionProjection {
        CollisionProjection::build(&VoxelWorld::new(spec()))
    }

    /// A wall of voxels at x = 3 (from 3 to 4), y and z 0..8.
    fn wall() -> CollisionProjection {
        let mut world = VoxelWorld::new(spec());
        let mut chunk = VoxelChunk::from_spec(&spec());
        for y in 0..8 {
            for z in 0..8 {
                chunk
                    .set(LocalVoxelCoord::new(3, y, z), VoxelValue::solid_raw(1))
                    .unwrap();
            }
        }
        world.insert(ChunkCoord::new(0, 0, 0), chunk);
        world.drain_dirty();
        CollisionProjection::build(&world)
    }

    fn body(entity: u64, part: u32, holder: u64, center: [f64; 3], half: f64) -> ContactTarget {
        ContactTarget {
            entity,
            part,
            holder,
            shape: ContactShape::Box {
                center: [0.0; 3],
                half_extents: [half; 3],
            },
            pose: ContactPose::at(center),
        }
    }

    fn entities(contacts: &[ShapeContact]) -> Vec<(u64, u32)> {
        contacts
            .iter()
            .filter_map(|contact| match contact.source {
                ContactSource::Entity { entity, part, .. } => Some((entity, part)),
                _ => None,
            })
            .collect()
    }

    /// A rotation of `angle` radians about +Y, `[x, y, z, w]`.
    fn yaw(angle: f64) -> [f64; 4] {
        [0.0, (angle / 2.0).sin(), 0.0, (angle / 2.0).cos()]
    }

    #[test]
    fn an_overlap_names_every_target_inside_in_entity_order() {
        let targets = [
            body(9, 0, 0, [1.0, 0.0, 0.0], 0.5),
            body(4, 0, 0, [-1.0, 0.0, 0.0], 0.5),
            body(7, 0, 0, [10.0, 0.0, 0.0], 0.5),
            body(4, 2, 0, [0.0, 1.0, 0.0], 0.25),
        ];
        let contacts = empty()
            .overlap_contacts(
                ContactShape::Sphere {
                    center: [0.0; 3],
                    radius: 1.0,
                },
                ContactPose::at([0.0; 3]),
                &targets,
                true,
            )
            .unwrap();
        assert_eq!(entities(&contacts), vec![(4, 0), (4, 2), (9, 0)]);
        assert!(contacts
            .iter()
            .all(|contact| contact.penetration_depth > 0.0));
    }

    #[test]
    fn an_overlap_reports_the_world_first_and_touching_is_not_overlapping() {
        let projection = wall();
        let shape = ContactShape::Box {
            center: [0.0; 3],
            half_extents: [0.5; 3],
        };
        let into_wall = projection
            .overlap_contacts(
                shape,
                ContactPose::at([2.75, 4.0, 4.0]),
                &[body(1, 0, 0, [2.0, 4.0, 4.0], 0.5)],
                true,
            )
            .unwrap();
        assert_eq!(into_wall[0].source, ContactSource::Voxel);
        assert!((into_wall[0].penetration_depth - 0.25).abs() < 1e-6);
        assert_eq!(entities(&into_wall), vec![(1, 0)]);
        let touching = projection
            .overlap_contacts(shape, ContactPose::at([2.5, 4.0, 4.0]), &[], true)
            .unwrap();
        assert!(touching.is_empty());
        let without_world = projection
            .overlap_contacts(shape, ContactPose::at([2.75, 4.0, 4.0]), &[], false)
            .unwrap();
        assert!(without_world.is_empty());
    }

    #[test]
    fn an_oriented_box_overlaps_by_its_rotation() {
        // A long thin box along X, turned a quarter about Y, lies along Z.
        let blade = ContactTarget {
            entity: 5,
            part: 1,
            holder: 0,
            shape: ContactShape::Box {
                center: [0.0; 3],
                half_extents: [2.0, 0.1, 0.1],
            },
            pose: ContactPose {
                translation: [0.0; 3],
                rotation: yaw(std::f64::consts::FRAC_PI_2),
            },
        };
        let probe = |at: [f64; 3]| {
            empty()
                .overlap_contacts(
                    ContactShape::Sphere {
                        center: [0.0; 3],
                        radius: 0.2,
                    },
                    ContactPose::at(at),
                    &[blade],
                    true,
                )
                .unwrap()
                .len()
        };
        assert_eq!(probe([0.0, 0.0, 1.5]), 1);
        assert_eq!(probe([1.5, 0.0, 0.0]), 0);
    }

    /// A blade: a capsule from the grip at the origin out 2 along +X.
    fn blade() -> ContactShape {
        ContactShape::Capsule {
            a: [0.0; 3],
            b: [2.0, 0.0, 0.0],
            radius: 0.05,
        }
    }

    #[test]
    fn a_turning_blade_strikes_a_held_shield_before_its_holder() {
        // The swing turns the blade a half turn about +Y at the grip, from +X
        // through -Z to -X. The defender stands at -Z with a shield before it.
        let defender = body(20, 0, 0, [0.0, 0.0, -1.8], 0.3);
        let shield = ContactTarget {
            entity: 21,
            part: 0,
            holder: 20,
            shape: ContactShape::Box {
                center: [0.0; 3],
                half_extents: [0.4, 0.4, 0.05],
            },
            pose: ContactPose {
                translation: [0.6, 0.0, -1.3],
                rotation: yaw(0.3),
            },
        };
        let contacts = empty()
            .sweep_contacts(
                blade(),
                ContactPose::at([0.0; 3]),
                ContactPose {
                    translation: [0.0; 3],
                    rotation: yaw(std::f64::consts::PI * 0.999),
                },
                [0.0; 3],
                &[defender, shield],
                true,
            )
            .unwrap();
        let order: Vec<_> = contacts.iter().map(|contact| contact.source).collect();
        assert_eq!(
            order,
            vec![
                ContactSource::Entity {
                    entity: 21,
                    part: 0,
                    holder: 20
                },
                ContactSource::Entity {
                    entity: 20,
                    part: 0,
                    holder: 0
                },
            ]
        );
        assert!(contacts[0].fraction < contacts[1].fraction);
        assert!(contacts.iter().all(|contact| !contact.start_solid));
        // The shield is struck on its +X side, the side the blade comes from.
        assert!(contacts[0].point[0] > 0.0);
    }

    #[test]
    fn a_sweep_meets_a_thin_target_between_its_end_poses() {
        let thin = ContactTarget {
            entity: 3,
            part: 7,
            holder: 0,
            shape: ContactShape::Box {
                center: [0.0; 3],
                half_extents: [0.02, 1.0, 1.0],
            },
            pose: ContactPose::at([5.0, 0.0, 0.0]),
        };
        let shape = ContactShape::Sphere {
            center: [0.0; 3],
            radius: 0.1,
        };
        let projection = empty();
        let overlaps = |x: f64| {
            projection
                .overlap_contacts(shape, ContactPose::at([x, 0.0, 0.0]), &[thin], true)
                .unwrap()
                .len()
        };
        assert_eq!((overlaps(0.0), overlaps(10.0)), (0, 0));
        let straight = projection
            .sweep_contacts(
                shape,
                ContactPose::at([0.0; 3]),
                ContactPose::at([10.0, 0.0, 0.0]),
                [0.0; 3],
                &[thin],
                true,
            )
            .unwrap();
        assert_eq!(entities(&straight), vec![(3, 7)]);
        assert!((straight[0].fraction - 0.488).abs() < 1e-3);
        assert!((straight[0].normal[0] + 1.0).abs() < 1e-6);
        // The same target, met by a blade tip turning past it.
        let turned = projection
            .sweep_contacts(
                ContactShape::Capsule {
                    a: [0.0; 3],
                    b: [6.0, 0.0, 0.0],
                    radius: 0.05,
                },
                ContactPose {
                    translation: [0.0; 3],
                    rotation: yaw(0.4),
                },
                ContactPose {
                    translation: [0.0; 3],
                    rotation: yaw(-0.4),
                },
                [0.0; 3],
                &[thin],
                true,
            )
            .unwrap();
        assert_eq!(entities(&turned), vec![(3, 7)]);
        assert!(turned[0].fraction > 0.0 && turned[0].fraction < 0.5);
    }

    #[test]
    fn world_geometry_struck_first_is_ordered_before_an_actor_behind_it() {
        let projection = wall();
        let behind = body(8, 0, 0, [5.0, 4.0, 4.0], 0.5);
        let before = body(6, 0, 0, [1.0, 4.0, 4.0], 0.25);
        let contacts = projection
            .sweep_contacts(
                ContactShape::Sphere {
                    center: [0.0; 3],
                    radius: 0.2,
                },
                ContactPose::at([0.0, 4.0, 4.0]),
                ContactPose::at([7.0, 4.0, 4.0]),
                [0.0; 3],
                &[behind, before],
                true,
            )
            .unwrap();
        let sources: Vec<_> = contacts.iter().map(|contact| contact.source).collect();
        assert_eq!(sources[0], before_source(6));
        assert_eq!(sources[1], ContactSource::Voxel);
        assert_eq!(sources[2], before_source(8));
        assert!((contacts[1].point[0] - 3.0).abs() < 1e-3);
        assert!((contacts[1].normal[0] + 1.0).abs() < 1e-3);
    }

    fn before_source(entity: u64) -> ContactSource {
        ContactSource::Entity {
            entity,
            part: 0,
            holder: 0,
        }
    }

    #[test]
    fn a_sweep_starting_inside_a_target_reports_it_solid_at_zero() {
        let contacts = empty()
            .sweep_contacts(
                ContactShape::Sphere {
                    center: [0.0; 3],
                    radius: 0.5,
                },
                ContactPose::at([0.0; 3]),
                ContactPose::at([3.0, 0.0, 0.0]),
                [0.0; 3],
                &[body(2, 0, 0, [0.0; 3], 0.5)],
                true,
            )
            .unwrap();
        assert_eq!(contacts.len(), 1);
        assert!(contacts[0].start_solid);
        assert_eq!(contacts[0].fraction, 0.0);
        assert!(contacts[0].penetration_depth > 0.0);
    }

    #[test]
    fn invalid_shapes_and_poses_are_refused() {
        let projection = empty();
        let refused = projection.overlap_contacts(
            ContactShape::Sphere {
                center: [0.0; 3],
                radius: 0.0,
            },
            ContactPose::at([0.0; 3]),
            &[],
            true,
        );
        assert_eq!(refused, Err(ContactQueryError::InvalidShape));
        let refused = projection.overlap_contacts(
            blade(),
            ContactPose {
                translation: [0.0; 3],
                rotation: [0.0; 4],
            },
            &[],
            true,
        );
        assert_eq!(refused, Err(ContactQueryError::InvalidPose));
    }
}
