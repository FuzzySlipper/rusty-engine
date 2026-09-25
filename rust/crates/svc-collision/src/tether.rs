use std::collections::{BTreeMap, BTreeSet};

use rapier3d_f64::prelude::{
    ImpulseJointHandle, PhysicsWorld, RigidBodyBuilder, RigidBodyHandle, RopeJointBuilder, Vector,
};

use crate::dynamics::{DynamicsBodyId, DynamicsBodyInput};

pub const MAX_DYNAMICS_TETHERS: usize = 576;
pub const TETHER_SUBSTEPS: usize = 4;
pub const TETHER_SOLVER_ITERATIONS: usize = 8;
pub const MAX_TETHER_REEL_SPEED: f64 = 0.25;
const TAUT_TOLERANCE: f64 = 0.001;

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
        if !(1..=8).contains(&self.substeps) || !(1..=16).contains(&self.iterations) {
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsTether {
    pub id: u64,
    pub first: DynamicsTetherEndpoint,
    pub second: DynamicsTetherEndpoint,
    pub maximum_length: f64,
    pub target_length: f64,
    pub reel_speed: f64,
    pub was_taut: bool,
    /// Wake attached bodies on creation and edits, not on every reconstructed tick.
    pub wake: bool,
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
    BudgetExceeded,
    InvalidSolverConfiguration,
    DuplicateId { id: u64 },
    InvalidDefinition { id: u64 },
    InvalidAnchor { id: u64 },
    OutputNotFinite { id: u64 },
}

impl DynamicsTetherError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidSolverConfiguration => "invalid-dynamics-rope-solver-configuration",
            Self::BudgetExceeded => "dynamics-tether-budget-exceeded",
            Self::DuplicateId { .. } => "duplicate-dynamics-tether",
            Self::InvalidDefinition { .. } => "invalid-dynamics-tether",
            Self::InvalidAnchor { .. } => "invalid-dynamics-tether-anchor",
            Self::OutputNotFinite { .. } => "non-finite-dynamics-tether-output",
        }
    }
}

pub fn validate_dynamics_tethers(
    tethers: &mut [DynamicsTether],
    bodies: &[DynamicsBodyInput],
) -> Result<(), DynamicsTetherError> {
    if tethers.len() > MAX_DYNAMICS_TETHERS {
        return Err(DynamicsTetherError::BudgetExceeded);
    }
    tethers.sort_by_key(|tether| tether.id);
    let enabled: BTreeSet<_> = bodies
        .iter()
        .filter(|body| body.enabled)
        .map(|body| body.id)
        .collect();
    let mut previous = None;
    for tether in tethers {
        let id = tether.id;
        if previous == Some(id) {
            return Err(DynamicsTetherError::DuplicateId { id });
        }
        previous = Some(id);
        if !tether.maximum_length.is_finite()
            || tether.maximum_length <= 0.0
            || !tether.target_length.is_finite()
            || tether.target_length <= 0.0
            || !tether.reel_speed.is_finite()
            || !(0.0..=MAX_TETHER_REEL_SPEED).contains(&tether.reel_speed)
        {
            return Err(DynamicsTetherError::InvalidDefinition { id });
        }
        let mut ids = Vec::new();
        for endpoint in [tether.first, tether.second] {
            let point = match endpoint {
                DynamicsTetherEndpoint::Fixed(point) => point,
                DynamicsTetherEndpoint::Body { body, local_anchor } => {
                    if !enabled.contains(&body) {
                        return Err(DynamicsTetherError::InvalidAnchor { id });
                    }
                    ids.push(body);
                    local_anchor
                }
            };
            if !point.into_iter().all(f64::is_finite) {
                return Err(DynamicsTetherError::InvalidAnchor { id });
            }
        }
        if ids.is_empty() || (ids.len() == 2 && ids[0] == ids[1]) {
            return Err(DynamicsTetherError::InvalidAnchor { id });
        }
    }
    Ok(())
}

pub(crate) struct SolverTether {
    definition: DynamicsTether,
    joint: ImpulseJointHandle,
    first: RigidBodyHandle,
    second: RigidBodyHandle,
    local_first: Vector,
    local_second: Vector,
    force_proxy: f64,
}

impl SolverTether {
    pub(crate) fn insert(
        definition: DynamicsTether,
        world: &mut PhysicsWorld,
        handles: &BTreeMap<DynamicsBodyId, RigidBodyHandle>,
    ) -> Self {
        let mut endpoint = |endpoint| match endpoint {
            DynamicsTetherEndpoint::Fixed(point) => (
                world.insert_body(RigidBodyBuilder::fixed().translation(Vector::from_array(point))),
                Vector::ZERO,
            ),
            DynamicsTetherEndpoint::Body { body, local_anchor } => {
                (handles[&body], Vector::from_array(local_anchor))
            }
        };
        let (first, local_first) = endpoint(definition.first);
        let (second, local_second) = endpoint(definition.second);
        let joint = world.impulse_joints.insert(
            first,
            second,
            RopeJointBuilder::new(definition.maximum_length)
                .local_anchor1(local_first)
                .local_anchor2(local_second)
                .contacts_enabled(definition.contacts_enabled),
            definition.wake,
        );
        Self {
            definition,
            joint,
            first,
            second,
            local_first,
            local_second,
            force_proxy: 0.0,
        }
    }

    pub(crate) fn reel(&mut self, world: &mut PhysicsWorld, seconds: f64) {
        let length = &mut self.definition.maximum_length;
        let change = (self.definition.target_length - *length).clamp(
            -self.definition.reel_speed * seconds,
            self.definition.reel_speed * seconds,
        );
        *length += change;
        if change != 0.0 {
            let joint = world
                .impulse_joints
                .get_mut(self.joint, true)
                .expect("admitted joint");
            joint.data.limits[0].max = *length;
        }
    }

    pub(crate) fn accumulate(&mut self, world: &PhysicsWorld) {
        let impulse = world
            .impulse_joints
            .get(self.joint)
            .expect("admitted joint")
            .data
            .limits[0]
            .impulse
            .abs();
        let solver_dt = world.integration_parameters.dt
            / world.integration_parameters.num_solver_iterations as f64;
        let force = impulse / solver_dt;
        if !force.is_finite() {
            self.force_proxy = force;
        } else if self.force_proxy.is_finite() {
            self.force_proxy = self.force_proxy.max(force);
        }
    }

    pub(crate) fn readout(
        &self,
        world: &PhysicsWorld,
    ) -> Result<DynamicsTetherReadout, DynamicsTetherError> {
        let first = world.bodies[self.first]
            .position()
            .transform_point(self.local_first);
        let second = world.bodies[self.second]
            .position()
            .transform_point(self.local_second);
        let distance = (second - first).length();
        if !first.is_finite()
            || !second.is_finite()
            || !distance.is_finite()
            || !self.force_proxy.is_finite()
        {
            return Err(DynamicsTetherError::OutputNotFinite {
                id: self.definition.id,
            });
        }
        let taut = distance >= self.definition.maximum_length - TAUT_TOLERANCE;
        Ok(DynamicsTetherReadout {
            id: self.definition.id,
            first: first.to_array(),
            second: second.to_array(),
            maximum_length: self.definition.maximum_length,
            distance,
            taut,
            caught: taut && !self.definition.was_taut,
            force_proxy: self.force_proxy,
        })
    }
}
