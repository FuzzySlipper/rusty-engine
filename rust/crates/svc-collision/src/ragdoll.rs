//! Ragdolls: one body per chosen bone, linked by limited joints, spawned at
//! an animated pose with its velocities, and read back as joint placements a
//! skeleton can follow.
//!
//! A bone's body spans from its joint to its tip. Joint frames come from the
//! rest pose, so a limit means the same turn whatever pose the ragdoll
//! spawns in.

use rapier3d_f64::prelude::{GenericJoint, Pose, Rotation, Vector};

use crate::dynamics::{DynamicsBodyId, DynamicsBodyInput, DynamicsShape, DynamicsSolver};
use crate::joint::{DynamicsJoint, DynamicsJointFrame, DynamicsJointLimit};
use crate::DynamicsError;

/// The shortest capsule core, so a bone shorter than its diameter still has
/// a defined axis.
const MINIMUM_HALF_HEIGHT: f64 = 1.0e-3;
/// Bones shorter than this have no direction.
const MINIMUM_BONE_LENGTH: f64 = 1.0e-4;

/// A rigid world placement of a joint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsPlacement {
    pub translation: [f64; 3],
    /// Quaternion in x/y/z/w order.
    pub rotation: [f64; 4],
}

impl DynamicsPlacement {
    /// `point` (world) in this placement's frame: a bone's tip from where its
    /// end joint is at rest.
    pub fn local_point(self, point: [f64; 3]) -> [f64; 3] {
        self.pose()
            .inverse_transform_point(Vector::from_array(point))
            .to_array()
    }

    fn pose(self) -> Pose {
        Pose::from_parts(
            Vector::from_array(self.translation),
            Rotation::from_array(self.rotation).normalize(),
        )
    }

    fn of(pose: Pose) -> Self {
        Self {
            translation: pose.translation.to_array(),
            rotation: pose.rotation.normalize().to_array(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RagdollShape {
    Capsule {
        radius: f64,
    },
    /// The box's length is the bone's; these are its other half extents.
    Box {
        half_width: f64,
        half_depth: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RagdollBone {
    pub body: DynamicsBodyId,
    /// The far end of the bone in its joint's frame at rest, in metres.
    pub tip: [f64; 3],
    pub shape: RagdollShape,
    pub mass: f64,
}

/// A limited joint at the child bone's joint. `axis` is the hinge or twist
/// axis in the child joint's frame at rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RagdollLink {
    pub joint: u64,
    pub parent: usize,
    pub child: usize,
    pub axis: [f64; 3],
    pub limit: DynamicsJointLimit,
    pub damping: f64,
}

/// What every body of a ragdoll shares.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RagdollMaterial {
    pub collision_groups: u32,
    pub collision_mask: u32,
    pub friction: f64,
    pub restitution: f64,
    pub linear_damping: f64,
    pub angular_damping: f64,
}

/// The joint placements a ragdoll spawns from, per bone: at rest, now, and
/// `elapsed` seconds before now (for the pose's velocities).
#[derive(Debug, Clone, PartialEq)]
pub struct RagdollSpawn {
    pub rest: Vec<DynamicsPlacement>,
    pub current: Vec<DynamicsPlacement>,
    pub previous: Option<(Vec<DynamicsPlacement>, f64)>,
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn at_least(value: f64, minimum: f64) -> bool {
    value.is_finite() && value >= minimum
}

/// The body frame relative to its bone's joint: halfway to the tip, with
/// the body's Y axis along the bone.
fn bone_offset(bone: &RagdollBone) -> Pose {
    let tip = Vector::from_array(bone.tip);
    Pose::from_parts(
        tip * 0.5,
        Rotation::from_rotation_arc(Vector::Y, tip.normalize()),
    )
}

fn bone_shape(bone: &RagdollBone) -> DynamicsShape {
    let half_length = Vector::from_array(bone.tip).length() * 0.5;
    match bone.shape {
        RagdollShape::Capsule { radius } => DynamicsShape::CapsuleY {
            half_height: (half_length - radius).max(MINIMUM_HALF_HEIGHT),
            radius,
        },
        RagdollShape::Box {
            half_width,
            half_depth,
        } => DynamicsShape::Cuboid {
            half_extents: [half_width, half_length, half_depth],
        },
    }
}

impl DynamicsSolver {
    /// Insert a ragdoll's bodies at `spawn.current`, moving as the pose
    /// moved, and link them. Every bone needs a placement in each pose. On a
    /// refusal nothing stays inserted.
    pub fn insert_ragdoll(
        &mut self,
        bones: &[RagdollBone],
        links: &[RagdollLink],
        material: RagdollMaterial,
        spawn: &RagdollSpawn,
    ) -> Result<(), DynamicsError> {
        let invalid = || DynamicsError::InvalidBody {
            body: bones.first().map_or(DynamicsBodyId(0), |bone| bone.body),
        };
        let complete = |placements: &[DynamicsPlacement]| placements.len() == bones.len();
        if !complete(&spawn.rest)
            || !complete(&spawn.current)
            || spawn
                .previous
                .as_ref()
                .is_some_and(|(previous, elapsed)| !complete(previous) || !positive(*elapsed))
            || bones
                .iter()
                .any(|bone| !at_least(Vector::from_array(bone.tip).length(), MINIMUM_BONE_LENGTH))
            || links.iter().any(|link| {
                link.parent >= bones.len()
                    || link.child >= bones.len()
                    || link.parent == link.child
                    || !positive(Vector::from_array(link.axis).length())
            })
        {
            return Err(invalid());
        }
        let offsets: Vec<Pose> = bones.iter().map(bone_offset).collect();
        let body_pose =
            |placements: &[DynamicsPlacement], bone: usize| placements[bone].pose() * offsets[bone];
        let mut inserted = Vec::new();
        let mut result = Ok(());
        for (index, bone) in bones.iter().enumerate() {
            let pose = body_pose(&spawn.current, index);
            let (linear, angular) = match &spawn.previous {
                Some((previous, elapsed)) => {
                    let before = body_pose(previous, index);
                    let turn = pose.rotation * before.rotation.inverse();
                    // The short way round.
                    let turn = if turn.w < 0.0 { -turn } else { turn };
                    (
                        (pose.translation - before.translation) / *elapsed,
                        turn.to_scaled_axis() / *elapsed,
                    )
                }
                None => (Vector::ZERO, Vector::ZERO),
            };
            result = self.insert_body(DynamicsBodyInput {
                id: bone.body,
                translation: pose.translation.to_array(),
                rotation: pose.rotation.normalize().to_array(),
                shape: bone_shape(bone),
                mass: bone.mass,
                mass_properties: None,
                linear_velocity: linear.to_array(),
                angular_velocity: angular.to_array(),
                locked_translation_axes: [false; 3],
                locked_rotation_axes: [false; 3],
                linear_damping: material.linear_damping,
                angular_damping: material.angular_damping,
                gravity_scale: 1.0,
                friction: material.friction,
                restitution: material.restitution,
                collision_groups: material.collision_groups,
                collision_mask: material.collision_mask,
                enabled: true,
                sleeping: false,
                continuous_collision: false,
            });
            if result.is_err() {
                break;
            }
            inserted.push(bone.body);
        }
        let mut joined = Vec::new();
        if result.is_ok() {
            for link in links {
                let frame = {
                    let rest = spawn.rest[link.child].pose();
                    let axis = Vector::from_array(link.axis).normalize();
                    Pose::from_parts(
                        rest.translation,
                        rest.rotation * GenericJoint::complete_ang_frame(axis),
                    )
                };
                let local = |bone: usize| {
                    DynamicsJointFrame::of(body_pose(&spawn.rest, bone).inverse() * frame)
                };
                result = self.set_joint(DynamicsJoint {
                    id: link.joint,
                    first: bones[link.parent].body,
                    second: bones[link.child].body,
                    first_frame: local(link.parent),
                    second_frame: local(link.child),
                    limit: link.limit,
                    damping: link.damping,
                    contacts_enabled: false,
                });
                if result.is_err() {
                    break;
                }
                joined.push(link.joint);
            }
        }
        if result.is_err() {
            for joint in joined {
                self.remove_joint(joint);
            }
            for body in inserted {
                self.remove_body(body);
            }
        }
        result
    }

    /// Where each bone's joint is, from its body. `None` for a bone whose
    /// body is gone.
    pub fn ragdoll_placements(&self, bones: &[RagdollBone]) -> Vec<Option<DynamicsPlacement>> {
        bones
            .iter()
            .map(|bone| {
                let body = self.body(bone.body)?;
                let pose = Pose::from_parts(
                    Vector::from_array(body.translation),
                    Rotation::from_array(body.rotation),
                );
                Some(DynamicsPlacement::of(pose * bone_offset(bone).inverse()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
    use core_voxel::VoxelValue;
    use svc_spatial::VoxelWorld;
    use svc_volume::VoxelChunk;

    use super::*;
    use crate::CollisionProjection;

    const TICK: f64 = 1.0 / 60.0;
    const GRAVITY: [f64; 3] = [0.0, -9.81, 0.0];
    const KNEE: u64 = 12;
    const ELBOW: u64 = 15;
    const KNEE_LIMIT: [f64; 2] = [0.0, 2.4];
    const ELBOW_LIMIT: [f64; 2] = [0.0, 2.5];
    /// Allowed overshoot of a limit while bodies collide (radians).
    const LIMIT_TOLERANCE: f64 = 0.05;

    /// A floor whose top is y = 0 over x and z in 0..16.
    fn floor() -> CollisionProjection {
        let spec = VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(8).unwrap()).unwrap();
        let mut world = VoxelWorld::new(spec);
        for (cx, cz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let mut chunk = VoxelChunk::from_spec(&spec);
            for x in 0..8 {
                for z in 0..8 {
                    chunk
                        .set(LocalVoxelCoord::new(x, 7, z), VoxelValue::solid_raw(1))
                        .unwrap();
                }
            }
            world.insert(ChunkCoord::new(cx, -1, cz), chunk);
        }
        world.drain_dirty();
        CollisionProjection::build(&world)
    }

    fn placement(translation: [f64; 3], rotation: Rotation) -> DynamicsPlacement {
        DynamicsPlacement {
            translation,
            rotation: rotation.to_array(),
        }
    }

    /// A standing figure: torso up from the hips, legs down, one arm out
    /// along -X. Joint frames have their bone along +Y.
    fn figure() -> (Vec<RagdollBone>, Vec<RagdollLink>, Vec<DynamicsPlacement>) {
        let up = Rotation::IDENTITY;
        let down = Rotation::from_rotation_z(std::f64::consts::PI);
        let out = Rotation::from_rotation_z(std::f64::consts::FRAC_PI_2);
        let capsule = |id: u64, length: f64, radius: f64, mass: f64| RagdollBone {
            body: DynamicsBodyId(id),
            tip: [0.0, length, 0.0],
            shape: RagdollShape::Capsule { radius },
            mass,
        };
        let bones = vec![
            RagdollBone {
                shape: RagdollShape::Box {
                    half_width: 0.18,
                    half_depth: 0.1,
                },
                ..capsule(1, 0.6, 0.0, 30.0)
            },
            capsule(2, 0.45, 0.07, 8.0),
            capsule(3, 0.45, 0.06, 4.0),
            capsule(4, 0.45, 0.07, 8.0),
            capsule(5, 0.45, 0.06, 4.0),
            capsule(6, 0.3, 0.05, 2.0),
            capsule(7, 0.28, 0.045, 1.5),
        ];
        let rest = vec![
            placement([8.0, 1.0, 8.0], up),
            placement([7.88, 1.0, 8.0], down),
            placement([7.88, 0.55, 8.0], down),
            placement([8.12, 1.0, 8.0], down),
            placement([8.12, 0.55, 8.0], down),
            placement([7.75, 1.5, 8.0], out),
            placement([7.45, 1.5, 8.0], out),
        ];
        let cone = |joint: u64, parent: usize, child: usize, swing: f64| RagdollLink {
            joint,
            parent,
            child,
            axis: [0.0, 1.0, 0.0],
            limit: DynamicsJointLimit::Cone {
                swing,
                twist_min: -0.3,
                twist_max: 0.3,
            },
            damping: 0.5,
        };
        let hinge = |joint: u64, parent: usize, child: usize, [min, max]: [f64; 2]| RagdollLink {
            joint,
            parent,
            child,
            axis: [0.0, 0.0, 1.0],
            limit: DynamicsJointLimit::Hinge { min, max },
            damping: 0.5,
        };
        let links = vec![
            cone(11, 0, 1, 0.8),
            hinge(KNEE, 1, 2, KNEE_LIMIT),
            cone(13, 0, 3, 0.8),
            hinge(KNEE + 2, 3, 4, KNEE_LIMIT),
            cone(16, 0, 5, 1.4),
            hinge(ELBOW, 5, 6, ELBOW_LIMIT),
        ];
        (bones, links, rest)
    }

    fn material() -> RagdollMaterial {
        RagdollMaterial {
            collision_groups: 1,
            collision_mask: u32::MAX,
            friction: 0.8,
            restitution: 0.0,
            linear_damping: 0.05,
            angular_damping: 0.5,
        }
    }

    /// The hinge's turn about its axis, from its two body frames.
    fn hinge_angle(solver: &DynamicsSolver, id: u64) -> f64 {
        let joint = solver.joint(id).unwrap();
        let frame = |body: DynamicsBodyId, frame: DynamicsJointFrame| {
            let body = solver.body(body).unwrap();
            Pose::from_parts(
                Vector::from_array(body.translation),
                Rotation::from_array(body.rotation),
            ) * frame.pose()
        };
        let first = frame(joint.first, joint.first_frame);
        let second = frame(joint.second, joint.second_frame);
        let relative = first.rotation.inverse() * second.rotation;
        2.0 * relative.x.atan2(relative.w)
    }

    struct Fall {
        bodies: Vec<[f64; 7]>,
        worst_limit: f64,
        lowest_surface: f64,
        resting_after: Option<usize>,
    }

    /// Spawn the figure, hit its chest sideways, and let it fall for
    /// `ticks`, watching the knee and elbow limits and the floor.
    fn fall(ticks: usize) -> Fall {
        let (bones, links, rest) = figure();
        let mut solver = DynamicsSolver::new(GRAVITY);
        solver.bind_environment(&floor());
        let spawn = RagdollSpawn {
            rest: rest.clone(),
            current: rest,
            previous: None,
        };
        solver
            .insert_ragdoll(&bones, &links, material(), &spawn)
            .unwrap();
        solver
            .apply_impulse_at_point(DynamicsBodyId(1), [8.0, 1.5, 8.0], [-60.0, 0.0, 25.0])
            .unwrap();
        let mut worst_limit = 0.0_f64;
        let mut lowest_surface = f64::MAX;
        let mut resting_after = None;
        for tick in 0..ticks {
            solver.step(TICK, 1, &[]).unwrap();
            for (joint, [min, max]) in [(KNEE, KNEE_LIMIT), (ELBOW, ELBOW_LIMIT)] {
                let angle = hinge_angle(&solver, joint);
                worst_limit = worst_limit.max(min - angle).max(angle - max);
            }
            for bone in &bones {
                let body = solver.body(bone.body).unwrap();
                let pose = Pose::from_parts(
                    Vector::from_array(body.translation),
                    Rotation::from_array(body.rotation),
                );
                let half = Vector::from_array(bone.tip).length() * 0.5;
                // The lowest point of the bone's segment, less its radius.
                let (radius, ends) = match bone.shape {
                    RagdollShape::Capsule { radius } => (radius, half - radius),
                    RagdollShape::Box { half_width, .. } => (half_width.min(0.1), half),
                };
                let low = [-ends, ends]
                    .into_iter()
                    .map(|y| pose.transform_point(Vector::new(0.0, y, 0.0)).y)
                    .fold(f64::MAX, f64::min);
                lowest_surface = lowest_surface.min(low - radius);
            }
            let resting = bones
                .iter()
                .all(|bone| solver.body(bone.body).unwrap().sleeping);
            if resting && resting_after.is_none() {
                resting_after = Some(tick);
            }
        }
        let bodies = bones
            .iter()
            .map(|bone| {
                let body = solver.body(bone.body).unwrap();
                let [x, y, z] = body.translation;
                let [qx, qy, qz, qw] = body.rotation;
                [x, y, z, qx, qy, qz, qw]
            })
            .collect();
        Fall {
            bodies,
            worst_limit,
            lowest_surface,
            resting_after,
        }
    }

    #[test]
    fn a_hit_figure_falls_rests_on_the_floor_within_its_limits_and_repeats_exactly() {
        let ticks = 60 * 8;
        let first = fall(ticks);
        assert!(
            first.worst_limit < LIMIT_TOLERANCE,
            "a hinge passed its limit by {} rad",
            first.worst_limit
        );
        assert!(
            first.lowest_surface > -0.03,
            "a bone sank {} m into the floor",
            -first.lowest_surface
        );
        let rest = first.resting_after.expect("the figure comes to rest");
        assert!(rest < ticks, "rested after {rest} ticks");
        assert!(
            first.bodies.iter().all(|body| body[1] < 0.5),
            "it fell: {:?}",
            first.bodies
        );
        let second = fall(ticks);
        assert_eq!(first.bodies, second.bodies, "same inputs, same fall");
    }

    #[test]
    fn spawned_bodies_carry_the_pose_velocity_and_the_joint_frames_meet() {
        let (bones, links, rest) = figure();
        let elapsed = 1.0 / 60.0;
        let velocity = [2.0, 0.0, -1.0];
        let previous: Vec<_> = rest
            .iter()
            .map(|placement| DynamicsPlacement {
                translation: [0, 1, 2]
                    .map(|axis| placement.translation[axis] - velocity[axis] * elapsed),
                ..*placement
            })
            .collect();
        let mut solver = DynamicsSolver::new([0.0; 3]);
        solver
            .insert_ragdoll(
                &bones,
                &links,
                material(),
                &RagdollSpawn {
                    rest: rest.clone(),
                    current: rest.clone(),
                    previous: Some((previous, elapsed)),
                },
            )
            .unwrap();
        for bone in &bones {
            let body = solver.body(bone.body).unwrap();
            for (moving, expected) in body.linear_velocity.iter().zip(velocity) {
                assert!((moving - expected).abs() < 1e-9);
            }
            assert!(body.angular_velocity.iter().all(|value| value.abs() < 1e-9));
        }
        // At rest every hinge reads zero and each bone's joint is where it was.
        assert!(hinge_angle(&solver, KNEE).abs() < 1e-9);
        let placed = solver.ragdoll_placements(&bones);
        for (placed, rest) in placed.iter().zip(&rest) {
            let placed = placed.unwrap();
            for axis in 0..3 {
                assert!((placed.translation[axis] - rest.translation[axis]).abs() < 1e-9);
            }
        }

        // A refused link leaves nothing behind.
        let mut solver = DynamicsSolver::new([0.0; 3]);
        let mut broken = links.clone();
        broken[3].child = 99;
        let spawn = RagdollSpawn {
            rest: rest.clone(),
            current: rest,
            previous: None,
        };
        assert!(solver
            .insert_ragdoll(&bones, &broken, material(), &spawn)
            .is_err());
        assert_eq!((solver.body_count(), solver.joint_count()), (0, 0));
    }
}
