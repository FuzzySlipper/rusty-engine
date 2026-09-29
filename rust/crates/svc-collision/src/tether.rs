use rapier3d_f64::prelude::{
    ImpulseJointHandle, PhysicsWorld, RigidBodyBuilder, RigidBodyHandle, RopeJointBuilder, Vector,
};

use crate::dynamics::DynamicsBodyId;

pub const TETHER_SUBSTEPS: usize = 4;
pub const TETHER_SOLVER_ITERATIONS: usize = 8;
/// Character-tether reel limit. Dynamics tethers take the product's reel speed.
pub const MAX_TETHER_REEL_SPEED: f64 = 0.25;
const TAUT_TOLERANCE: f64 = 0.001;
/// A new attachment may start this much beyond its length. Further than that,
/// the rope joint would yank the bodies together in one step.
const REACH_TOLERANCE: f64 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicsRopeSolverConfig {
    pub substeps: usize,
    pub iterations: usize,
}

impl Default for DynamicsRopeSolverConfig {
    fn default() -> Self {
        Self {
            substeps: TETHER_SUBSTEPS,
            iterations: TETHER_SOLVER_ITERATIONS,
        }
    }
}

impl DynamicsRopeSolverConfig {
    pub fn validate(self) -> Result<(), DynamicsTetherError> {
        if self.substeps == 0 || self.iterations == 0 {
            return Err(DynamicsTetherError::InvalidSolverConfiguration);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DynamicsTetherEndpoint {
    Fixed([f64; 3]),
    Body {
        body: DynamicsBodyId,
        local_anchor: [f64; 3],
    },
}

impl DynamicsTetherEndpoint {
    pub fn body(self) -> Option<DynamicsBodyId> {
        match self {
            Self::Fixed(_) => None,
            Self::Body { body, .. } => Some(body),
        }
    }
}

/// A maximum-distance rope. `maximum_length` is the current effective length;
/// it moves toward `target_length` at `reel_speed` metres per second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsTether {
    pub id: u64,
    pub first: DynamicsTetherEndpoint,
    pub second: DynamicsTetherEndpoint,
    pub maximum_length: f64,
    pub target_length: f64,
    pub reel_speed: f64,
    pub contacts_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsTetherReadout {
    pub id: u64,
    pub first: [f64; 3],
    pub second: [f64; 3],
    pub maximum_length: f64,
    pub distance: f64,
    pub taut: bool,
    pub caught: bool,
    /// Maximum sampled terminal solver-substep force, in N. May miss catch peaks.
    pub force_proxy: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DynamicsTetherError {
    InvalidSolverConfiguration,
    InvalidDefinition { id: u64 },
    InvalidAnchor { id: u64 },
    OutOfReach { id: u64 },
}

impl DynamicsTetherError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidSolverConfiguration => "invalid-dynamics-rope-solver-configuration",
            Self::InvalidDefinition { .. } => "invalid-dynamics-tether",
            Self::InvalidAnchor { .. } => "invalid-dynamics-tether-anchor",
            Self::OutOfReach { .. } => "dynamics-tether-out-of-reach",
        }
    }
}

/// One rope joint retained in the solver world. A fixed endpoint is a fixed
/// anchor body the tether owns.
pub(crate) struct SolverTether {
    pub(crate) definition: DynamicsTether,
    joint: ImpulseJointHandle,
    first: RigidBodyHandle,
    second: RigidBodyHandle,
    local_first: Vector,
    local_second: Vector,
    was_taut: bool,
    force_proxy: f64,
}

impl SolverTether {
    pub(crate) fn validate(definition: &DynamicsTether) -> Result<(), DynamicsTetherError> {
        let id = definition.id;
        if !definition.maximum_length.is_finite()
            || definition.maximum_length <= 0.0
            || !definition.target_length.is_finite()
            || definition.target_length <= 0.0
            || !definition.reel_speed.is_finite()
            || definition.reel_speed < 0.0
        {
            return Err(DynamicsTetherError::InvalidDefinition { id });
        }
        for endpoint in [definition.first, definition.second] {
            let point = match endpoint {
                DynamicsTetherEndpoint::Fixed(point) => point,
                DynamicsTetherEndpoint::Body { local_anchor, .. } => local_anchor,
            };
            if !point.into_iter().all(f64::is_finite) {
                return Err(DynamicsTetherError::InvalidAnchor { id });
            }
        }
        if definition.first.body().is_some() && definition.first.body() == definition.second.body()
        {
            return Err(DynamicsTetherError::InvalidAnchor { id });
        }
        Ok(())
    }

    /// The current distance between the two endpoints, before any joint exists.
    pub(crate) fn endpoint_distance(
        definition: &DynamicsTether,
        world: &PhysicsWorld,
        resolve: impl Fn(DynamicsBodyId) -> Option<RigidBodyHandle>,
    ) -> Result<f64, DynamicsTetherError> {
        let point = |endpoint| match endpoint {
            DynamicsTetherEndpoint::Fixed(point) => Ok(Vector::from_array(point)),
            DynamicsTetherEndpoint::Body { body, local_anchor } => resolve(body)
                .map(|handle| {
                    world.bodies[handle]
                        .position()
                        .transform_point(Vector::from_array(local_anchor))
                })
                .ok_or(DynamicsTetherError::InvalidAnchor { id: definition.id }),
        };
        Ok((point(definition.second)? - point(definition.first)?).length())
    }

    pub(crate) fn reach_exceeded(definition: &DynamicsTether, distance: f64) -> bool {
        !distance.is_finite() || distance > definition.maximum_length + REACH_TOLERANCE
    }

    pub(crate) fn insert(
        definition: DynamicsTether,
        was_taut: bool,
        world: &mut PhysicsWorld,
        resolve: impl Fn(DynamicsBodyId) -> Option<RigidBodyHandle>,
    ) -> Result<Self, DynamicsTetherError> {
        let id = definition.id;
        let mut endpoint = |endpoint| match endpoint {
            DynamicsTetherEndpoint::Fixed(point) => Ok((
                world.insert_body(RigidBodyBuilder::fixed().translation(Vector::from_array(point))),
                Vector::ZERO,
            )),
            DynamicsTetherEndpoint::Body { body, local_anchor } => resolve(body)
                .map(|handle| (handle, Vector::from_array(local_anchor)))
                .ok_or(DynamicsTetherError::InvalidAnchor { id }),
        };
        let (first, local_first) = endpoint(definition.first)?;
        let (second, local_second) = match endpoint(definition.second) {
            Ok(value) => value,
            Err(error) => {
                if definition.first.body().is_none() {
                    world.remove_body(first);
                }
                return Err(error);
            }
        };
        let joint = world.impulse_joints.insert(
            first,
            second,
            RopeJointBuilder::new(definition.maximum_length)
                .local_anchor1(local_first)
                .local_anchor2(local_second)
                .contacts_enabled(definition.contacts_enabled),
            true,
        );
        Ok(Self {
            definition,
            joint,
            first,
            second,
            local_first,
            local_second,
            was_taut,
            force_proxy: 0.0,
        })
    }

    /// Detach from the world, releasing any fixed anchor body. Rapier already
    /// removed the joint if one of its bodies was removed.
    pub(crate) fn remove(self, world: &mut PhysicsWorld) -> (DynamicsTether, bool) {
        world.impulse_joints.remove(self.joint, true);
        if self.definition.first.body().is_none() {
            world.remove_body(self.first);
        }
        if self.definition.second.body().is_none() {
            world.remove_body(self.second);
        }
        (self.definition, self.was_taut)
    }

    /// Change the reel target, speed or contacts of the existing joint.
    pub(crate) fn edit(&mut self, world: &mut PhysicsWorld, definition: DynamicsTether) {
        self.definition.target_length = definition.target_length;
        self.definition.reel_speed = definition.reel_speed;
        self.definition.contacts_enabled = definition.contacts_enabled;
        if let Some(joint) = world.impulse_joints.get_mut(self.joint, true) {
            joint.data.set_contacts_enabled(definition.contacts_enabled);
        }
    }

    pub(crate) fn translate(&mut self, world: &mut PhysicsWorld, delta: Vector) {
        for (endpoint, handle) in [
            (&mut self.definition.first, self.first),
            (&mut self.definition.second, self.second),
        ] {
            if let DynamicsTetherEndpoint::Fixed(point) = endpoint {
                let moved = Vector::from_array(*point) + delta;
                *point = moved.to_array();
                world.bodies[handle].set_translation(moved, false);
            }
        }
    }

    pub(crate) fn begin_step(&mut self) {
        self.force_proxy = 0.0;
    }

    pub(crate) fn reel(&mut self, world: &mut PhysicsWorld, seconds: f64) {
        let length = &mut self.definition.maximum_length;
        let change = (self.definition.target_length - *length).clamp(
            -self.definition.reel_speed * seconds,
            self.definition.reel_speed * seconds,
        );
        *length += change;
        if change != 0.0 {
            if let Some(joint) = world.impulse_joints.get_mut(self.joint, true) {
                joint.data.limits[0].max = *length;
            }
        }
    }

    pub(crate) fn accumulate(&mut self, world: &PhysicsWorld) {
        let Some(joint) = world.impulse_joints.get(self.joint) else {
            return;
        };
        let impulse = joint.data.limits[0].impulse.abs();
        let solver_dt = world.integration_parameters.dt
            / world.integration_parameters.num_solver_iterations as f64;
        self.force_proxy = self.force_proxy.max(impulse / solver_dt);
    }

    /// Read the rope after a step. Records tautness so the next catch is a
    /// transition from slack.
    pub(crate) fn readout(&mut self, world: &PhysicsWorld) -> DynamicsTetherReadout {
        let first = world.bodies[self.first]
            .position()
            .transform_point(self.local_first);
        let second = world.bodies[self.second]
            .position()
            .transform_point(self.local_second);
        let distance = (second - first).length();
        let taut = distance >= self.definition.maximum_length - TAUT_TOLERANCE;
        let caught = taut && !self.was_taut;
        self.was_taut = taut;
        DynamicsTetherReadout {
            id: self.definition.id,
            first: first.to_array(),
            second: second.to_array(),
            maximum_length: self.definition.maximum_length,
            distance,
            taut,
            caught,
            force_proxy: self.force_proxy,
        }
    }
}
