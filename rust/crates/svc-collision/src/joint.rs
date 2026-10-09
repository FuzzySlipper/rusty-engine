//! Limited joints between two Dynamics bodies: a hinge, or a cone with
//! twist, each with optional damping. Ragdolls link their bones with them.

use rapier3d_f64::prelude::{
    GenericJointBuilder, ImpulseJointHandle, JointAxesMask, JointAxis, PhysicsWorld, Pose,
    RigidBodyHandle, Rotation, Vector,
};

use crate::dynamics::DynamicsBodyId;

/// A body-local joint frame: where the joint sits on the body, and its axes.
/// Its X axis is the hinge or twist axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsJointFrame {
    pub anchor: [f64; 3],
    /// Quaternion in x/y/z/w order.
    pub rotation: [f64; 4],
}

impl DynamicsJointFrame {
    pub(crate) fn pose(self) -> Pose {
        Pose::from_parts(
            Vector::from_array(self.anchor),
            Rotation::from_array(self.rotation).normalize(),
        )
    }

    pub(crate) fn of(pose: Pose) -> Self {
        Self {
            anchor: pose.translation.to_array(),
            rotation: pose.rotation.normalize().to_array(),
        }
    }
}

/// How the two frames may turn relative to each other. Angles are radians;
/// zero is where the two frames coincide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DynamicsJointLimit {
    /// Turns about the frames' X axis only, between `min` and `max`.
    Hinge { min: f64, max: f64 },
    /// The second frame's X axis stays within `swing` of the first's, and
    /// turns about it between `twist_min` and `twist_max`.
    Cone {
        swing: f64,
        twist_min: f64,
        twist_max: f64,
    },
}

/// A limited joint. Both frames are body-local. `damping` (N·m·s per
/// radian) resists turning about the hinge or twist axis, so limbs settle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsJoint {
    pub id: u64,
    pub first: DynamicsBodyId,
    pub second: DynamicsBodyId,
    pub first_frame: DynamicsJointFrame,
    pub second_frame: DynamicsJointFrame,
    pub limit: DynamicsJointLimit,
    pub damping: f64,
    /// Whether the two bodies collide with each other.
    pub contacts_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DynamicsJointError {
    Invalid { id: u64 },
    SameBody { id: u64 },
    UnknownBody { id: u64, body: DynamicsBodyId },
}

impl DynamicsJointError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Invalid { .. } => "invalid-dynamics-joint",
            Self::SameBody { .. } => "dynamics-joint-same-body",
            Self::UnknownBody { .. } => "dynamics-joint-unknown-body",
        }
    }
}

pub(crate) struct SolverJoint {
    pub(crate) definition: DynamicsJoint,
    handle: ImpulseJointHandle,
}

impl SolverJoint {
    pub(crate) fn validate(joint: &DynamicsJoint) -> Result<(), DynamicsJointError> {
        let invalid = DynamicsJointError::Invalid { id: joint.id };
        if joint.first == joint.second {
            return Err(DynamicsJointError::SameBody { id: joint.id });
        }
        let frames = [joint.first_frame, joint.second_frame];
        let finite = frames
            .iter()
            .flat_map(|frame| frame.anchor.into_iter().chain(frame.rotation))
            .chain([joint.damping])
            .all(f64::is_finite);
        let unit = frames.iter().all(|frame| {
            let length = frame
                .rotation
                .iter()
                .map(|value| value * value)
                .sum::<f64>();
            (length - 1.0).abs() <= 1.0e-3
        });
        let angle = |value: f64| value.is_finite() && value.abs() <= std::f64::consts::PI;
        let limits = match joint.limit {
            DynamicsJointLimit::Hinge { min, max } => angle(min) && angle(max) && min <= max,
            DynamicsJointLimit::Cone {
                swing,
                twist_min,
                twist_max,
            } => {
                angle(swing)
                    && swing >= 0.0
                    && angle(twist_min)
                    && angle(twist_max)
                    && twist_min <= twist_max
            }
        };
        if !finite || !unit || !limits || joint.damping < 0.0 {
            return Err(invalid);
        }
        Ok(())
    }

    pub(crate) fn insert(
        definition: DynamicsJoint,
        world: &mut PhysicsWorld,
        first: RigidBodyHandle,
        second: RigidBodyHandle,
    ) -> Self {
        let builder = match definition.limit {
            DynamicsJointLimit::Hinge { min, max } => {
                GenericJointBuilder::new(JointAxesMask::LOCKED_REVOLUTE_AXES)
                    .limits(JointAxis::AngX, [min, max])
            }
            DynamicsJointLimit::Cone {
                swing,
                twist_min,
                twist_max,
            } => GenericJointBuilder::new(JointAxesMask::LOCKED_SPHERICAL_AXES)
                // Coupled swing axes make a round cone about X.
                .coupled_axes(JointAxesMask::ANG_Y | JointAxesMask::ANG_Z)
                .limits(JointAxis::AngY, [0.0, swing])
                .limits(JointAxis::AngX, [twist_min, twist_max]),
        };
        let builder = builder
            .local_frame1(definition.first_frame.pose())
            .local_frame2(definition.second_frame.pose())
            .contacts_enabled(definition.contacts_enabled);
        let builder = if definition.damping > 0.0 {
            builder.motor_velocity(JointAxis::AngX, 0.0, definition.damping)
        } else {
            builder
        };
        let handle = world.impulse_joints.insert(first, second, builder, true);
        Self { definition, handle }
    }

    /// Detach from the world. Rapier already removed the joint if one of its
    /// bodies was removed.
    pub(crate) fn remove(self, world: &mut PhysicsWorld) -> DynamicsJoint {
        world.impulse_joints.remove(self.handle, true);
        self.definition
    }

    pub(crate) fn attaches(&self, body: DynamicsBodyId) -> bool {
        self.definition.first == body || self.definition.second == body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DynamicsAction, DynamicsBodyInput, DynamicsShape, DynamicsSolver};

    const TICK: f64 = 1.0 / 60.0;

    fn rod(id: u64, y: f64, pinned: bool) -> DynamicsBodyInput {
        DynamicsBodyInput {
            id: DynamicsBodyId(id),
            translation: [0.0, y, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            shape: DynamicsShape::CapsuleY {
                half_height: 0.4,
                radius: 0.05,
            },
            mass: 1.0,
            mass_properties: None,
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            locked_translation_axes: [pinned; 3],
            locked_rotation_axes: [pinned; 3],
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 1.0,
            friction: 0.5,
            restitution: 0.0,
            collision_groups: u32::MAX,
            collision_mask: u32::MAX,
            enabled: true,
            sleeping: false,
            continuous_collision: false,
        }
    }

    /// A pinned rod with a second rod hanging from its lower end. A hinge's X
    /// axis is along world Z, so it swings in the XY plane; a cone's is along
    /// the rods.
    fn hanging(limit: DynamicsJointLimit, damping: f64) -> DynamicsSolver {
        let axes = match limit {
            DynamicsJointLimit::Hinge { .. } => {
                Rotation::from_rotation_y(-std::f64::consts::FRAC_PI_2)
            }
            DynamicsJointLimit::Cone { .. } => {
                Rotation::from_rotation_z(std::f64::consts::FRAC_PI_2)
            }
        };
        let mut solver = DynamicsSolver::new([0.0, -9.81, 0.0]);
        solver.insert_body(rod(1, 1.0, true)).unwrap();
        solver.insert_body(rod(2, 0.0, false)).unwrap();
        let frame = |y: f64| DynamicsJointFrame {
            anchor: [0.0, y, 0.0],
            rotation: axes.to_array(),
        };
        solver
            .set_joint(DynamicsJoint {
                id: 1,
                first: DynamicsBodyId(1),
                second: DynamicsBodyId(2),
                first_frame: frame(-0.5),
                second_frame: frame(0.5),
                limit,
                damping,
                contacts_enabled: false,
            })
            .unwrap();
        solver
    }

    /// The hanging rod's angle from straight down, in radians.
    fn swing(solver: &DynamicsSolver) -> f64 {
        let rod = solver.body(DynamicsBodyId(2)).unwrap();
        let axis = Rotation::from_array(rod.rotation) * Vector::Y;
        axis.y.clamp(-1.0, 1.0).acos()
    }

    fn kick(solver: &mut DynamicsSolver) {
        solver
            .step(
                TICK,
                1,
                &[DynamicsAction::impulse(DynamicsBodyId(2), [6.0, 0.0, 0.0])],
            )
            .unwrap();
    }

    #[test]
    fn a_hinge_stops_at_its_limit_and_damping_settles_it() {
        let limit = DynamicsJointLimit::Hinge {
            min: -0.5,
            max: 0.5,
        };
        let mut solver = hanging(limit, 0.0);
        kick(&mut solver);
        let mut widest = 0.0_f64;
        for _ in 0..120 {
            solver.step(TICK, 1, &[]).unwrap();
            widest = widest.max(swing(&solver));
        }
        assert!(widest > 0.45 && widest < 0.52, "swung to {widest} rad");
        // It still swings undamped; damped, it has settled.
        let undamped = solver.body(DynamicsBodyId(2)).unwrap().angular_velocity;
        let mut damped = hanging(limit, 2.0);
        kick(&mut damped);
        for _ in 0..600 {
            damped.step(TICK, 1, &[]).unwrap();
        }
        let settled = damped.body(DynamicsBodyId(2)).unwrap().angular_velocity;
        let speed = |value: [f64; 3]| Vector::from_array(value).length();
        assert!(speed(undamped) > 0.5, "undamped {undamped:?}");
        assert!(speed(settled) < 0.05, "damped {settled:?}");
    }

    #[test]
    fn a_cone_holds_its_swing_and_joints_follow_body_replacement_and_removal() {
        let mut solver = hanging(
            DynamicsJointLimit::Cone {
                swing: 0.3,
                twist_min: -0.1,
                twist_max: 0.1,
            },
            0.0,
        );
        kick(&mut solver);
        let mut widest = 0.0_f64;
        for _ in 0..120 {
            solver.step(TICK, 1, &[]).unwrap();
            widest = widest.max(swing(&solver));
        }
        assert!(widest > 0.25 && widest < 0.32, "swung to {widest} rad");

        solver.replace_body(rod(2, 0.0, false)).unwrap();
        assert_eq!(solver.joint_count(), 1, "replacing a body keeps its joints");
        solver.remove_body(DynamicsBodyId(2));
        assert_eq!(solver.joint_count(), 0, "removing a body drops its joints");
        assert_eq!(
            solver.set_joint(DynamicsJoint {
                id: 2,
                first: DynamicsBodyId(1),
                second: DynamicsBodyId(1),
                first_frame: DynamicsJointFrame::of(Pose::IDENTITY),
                second_frame: DynamicsJointFrame::of(Pose::IDENTITY),
                limit: DynamicsJointLimit::Hinge { min: 0.0, max: 0.0 },
                damping: 0.0,
                contacts_enabled: false,
            }),
            Err(crate::DynamicsError::Joint(DynamicsJointError::SameBody {
                id: 2
            }))
        );
    }
}
