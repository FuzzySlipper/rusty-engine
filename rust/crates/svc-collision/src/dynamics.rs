use std::collections::BTreeMap;
use std::sync::Arc;

use rapier3d_f64::prelude::{
    ColliderBuilder, ColliderHandle, Group, IntegrationParameters, InteractionGroups,
    InteractionTestMode, LockedAxes, MassProperties, PhysicsWorld, Pose, RigidBodyBuilder,
    RigidBodyHandle, RigidBodyType, Rotation, SharedShape, Vector,
};

use crate::joint::{DynamicsJoint, DynamicsJointError, DynamicsJointMotor, SolverJoint};
use crate::soft_body::{
    soft_id, DynamicsSoftBodyError, DynamicsSoftBodyId, DynamicsSoftBodySettings, SolverSoftBody,
    TearEvents,
};
use crate::tether::{
    DynamicsRopeSolverConfig, DynamicsTether, DynamicsTetherError, DynamicsTetherReadout,
    SolverTether,
};
use crate::water::{shape_volume, submerged, DynamicsWaterPlane, DynamicsWaterReadout};
use crate::CollisionProjection;

const MASS_PROPERTIES_FRAME_NORMALIZATION_TOLERANCE: f64 = 1.0e-3;
/// Rolling resistance as a share of gravity (`DynamicsSolver::resist_rolling`):
/// under Earth's gravity a rolling sphere or capsule's surface speed loses
/// 0.2 m/s every second, which brings a 2 m/s solid ball to rest in about
/// 30 m.
const ROLLING_RESISTANCE: f64 = 0.02;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DynamicsBodyId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DynamicsShape {
    Sphere { radius: f64 },
    Cuboid { half_extents: [f64; 3] },
    CapsuleY { half_height: f64, radius: f64 },
}

/// Authored mass properties for one dynamics body.
///
/// The body's `mass` remains the sole authoritative total mass. This tuple
/// supplies the local center of mass, principal inertia, and the frame in
/// which that diagonal inertia is expressed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsMassProperties {
    pub center_of_mass: [f64; 3],
    pub principal_inertia: [f64; 3],
    pub principal_inertia_local_frame: [f64; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsBodyInput {
    pub id: DynamicsBodyId,
    pub translation: [f64; 3],
    /// Quaternion in x/y/z/w order.
    pub rotation: [f64; 4],
    pub shape: DynamicsShape,
    pub mass: f64,
    pub mass_properties: Option<DynamicsMassProperties>,
    pub linear_velocity: [f64; 3],
    pub angular_velocity: [f64; 3],
    /// `true` locks the corresponding world-space X/Y/Z translation axis.
    pub locked_translation_axes: [bool; 3],
    /// `true` locks the corresponding world-space X/Y/Z rotation axis.
    pub locked_rotation_axes: [bool; 3],
    pub linear_damping: f64,
    pub angular_damping: f64,
    pub gravity_scale: f64,
    pub friction: f64,
    pub restitution: f64,
    pub collision_groups: u32,
    pub collision_mask: u32,
    pub enabled: bool,
    pub sleeping: bool,
    pub continuous_collision: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsAction {
    pub body: DynamicsBodyId,
    pub force: [f64; 3],
    pub torque: [f64; 3],
    pub impulse: [f64; 3],
    pub torque_impulse: [f64; 3],
    pub wake: bool,
}

impl DynamicsAction {
    pub const fn impulse(body: DynamicsBodyId, impulse: [f64; 3]) -> Self {
        Self {
            body,
            force: [0.0; 3],
            torque: [0.0; 3],
            impulse,
            torque_impulse: [0.0; 3],
            wake: true,
        }
    }

    fn is_finite(&self) -> bool {
        self.force
            .into_iter()
            .chain(self.torque)
            .chain(self.impulse)
            .chain(self.torque_impulse)
            .all(f64::is_finite)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsBodyOutput {
    pub id: DynamicsBodyId,
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
    pub linear_velocity: [f64; 3],
    pub angular_velocity: [f64; 3],
    pub sleeping: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsContact {
    pub first: DynamicsBodyId,
    pub second: Option<DynamicsBodyId>,
    pub impulse: [f64; 3],
    pub impulse_magnitude: f64,
}

/// Counts from one [`DynamicsSolver::step`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicsStepReceipt {
    pub generation: u64,
    pub body_count: usize,
    pub contact_count: usize,
    pub rope_link_count: usize,
    pub rope_substeps: usize,
    pub rope_iterations: usize,
}

/// What a static-environment rebind changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DynamicsEnvironmentReceipt {
    pub retained: usize,
    pub inserted: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DynamicsError {
    Tether(DynamicsTetherError),
    Joint(DynamicsJointError),
    InvalidStep { actual: f64 },
    DuplicateBody { body: DynamicsBodyId },
    UnknownBody { body: DynamicsBodyId },
    InvalidBody { body: DynamicsBodyId },
    InvalidAction { body: DynamicsBodyId },
    InvalidWater { plane: usize },
    SoftBody(DynamicsSoftBodyError),
}

impl DynamicsError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Tether(error) => error.code(),
            Self::Joint(error) => error.code(),
            Self::InvalidStep { .. } => "invalid-dynamics-step",
            Self::DuplicateBody { .. } => "duplicate-dynamics-body",
            Self::UnknownBody { .. } => "unknown-dynamics-body",
            Self::InvalidBody { .. } => "invalid-dynamics-body",
            Self::InvalidAction { .. } => "invalid-dynamics-action",
            Self::InvalidWater { .. } => "invalid-dynamics-water",
            Self::SoftBody(error) => error.code(),
        }
    }
}

impl std::fmt::Display for DynamicsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.code())
    }
}

impl std::error::Error for DynamicsError {}

impl From<DynamicsJointError> for DynamicsError {
    fn from(value: DynamicsJointError) -> Self {
        Self::Joint(value)
    }
}

impl From<DynamicsTetherError> for DynamicsError {
    fn from(value: DynamicsTetherError) -> Self {
        Self::Tether(value)
    }
}

/// Linear point-velocity response to unit world impulses, including locked
/// axes and rotational inertia.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsAnchorObservation {
    pub point: [f64; 3],
    pub point_velocity: [f64; 3],
    pub center_of_mass: [f64; 3],
    pub response: [[f64; 3]; 3],
}

/// One live Rapier world. Bodies, static environment colliders, rope joints
/// and limited joints persist between steps, so contacts, sleeping and solver
/// warm starts carry over. Changes apply directly; nothing is staged or
/// rolled back.
pub struct DynamicsSolver {
    pub(crate) world: PhysicsWorld,
    bodies: BTreeMap<DynamicsBodyId, RigidBodyHandle>,
    /// Static colliders keyed by the address of their shared shape, so a
    /// rebind replaces only the chunks and mesh instances that changed.
    environment: BTreeMap<usize, (SharedShape, ColliderHandle)>,
    tethers: BTreeMap<u64, SolverTether>,
    joints: BTreeMap<u64, SolverJoint>,
    rope_solver: DynamicsRopeSolverConfig,
    contacts: Vec<DynamicsContact>,
    tether_readouts: Vec<DynamicsTetherReadout>,
    /// Every body's shape, for the water to measure.
    shapes: BTreeMap<DynamicsBodyId, DynamicsShape>,
    water: Vec<DynamicsWaterPlane>,
    /// The bodies the water acts on.
    floating: BTreeMap<DynamicsBodyId, Floating>,
    /// Where kinematic bodies are to be by the end of the next step.
    kinematic_targets: BTreeMap<DynamicsBodyId, Pose>,
    pub(crate) soft_bodies: BTreeMap<DynamicsSoftBodyId, SolverSoftBody>,
    pub(crate) soft_settings: DynamicsSoftBodySettings,
    generation: u64,
}

impl DynamicsSolver {
    pub fn new(gravity: [f64; 3]) -> Self {
        Self {
            world: PhysicsWorld {
                gravity: vector(gravity),
                ..PhysicsWorld::default()
            },
            bodies: BTreeMap::new(),
            environment: BTreeMap::new(),
            tethers: BTreeMap::new(),
            joints: BTreeMap::new(),
            rope_solver: DynamicsRopeSolverConfig::default(),
            contacts: Vec::new(),
            tether_readouts: Vec::new(),
            shapes: BTreeMap::new(),
            water: Vec::new(),
            floating: BTreeMap::new(),
            kinematic_targets: BTreeMap::new(),
            soft_bodies: BTreeMap::new(),
            soft_settings: DynamicsSoftBodySettings::default(),
            generation: 0,
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// Contacts from the latest step, ordered by body identity.
    pub fn contacts(&self) -> &[DynamicsContact] {
        &self.contacts
    }

    /// Rope readouts from the latest step.
    pub fn tether_readouts(&self) -> &[DynamicsTetherReadout] {
        &self.tether_readouts
    }

    /// Make `projection` the static environment. Colliders whose shape is
    /// unchanged stay in the world; bodies touching a removed collider wake.
    pub fn bind_environment(
        &mut self,
        projection: &CollisionProjection,
    ) -> DynamicsEnvironmentReceipt {
        let mut receipt = DynamicsEnvironmentReceipt::default();
        let mut next = BTreeMap::new();
        for shape in projection.dynamics_shapes() {
            let key = shape_key(&shape);
            if next.contains_key(&key) {
                continue;
            }
            let entry = match self.environment.remove(&key) {
                Some(entry) => {
                    receipt.retained += 1;
                    entry
                }
                None => {
                    receipt.inserted += 1;
                    let collider = self.world.insert_collider(
                        ColliderBuilder::new(shape.clone())
                            // Fixed terrain contributes no dynamic mass. Avoid
                            // walking every voxel child to derive its inertia.
                            .mass_properties(MassProperties::default())
                            .collision_groups(InteractionGroups::all())
                            .user_data(0),
                        None,
                    );
                    (shape, collider)
                }
            };
            next.insert(key, entry);
        }
        for (_, (_, collider)) in std::mem::replace(&mut self.environment, next) {
            receipt.removed += 1;
            let touching = self
                .world
                .contact_pairs_with(collider)
                .flat_map(|pair| [pair.collider1, pair.collider2])
                .filter_map(|handle| self.world.colliders.get(handle)?.parent())
                .collect::<Vec<_>>();
            for body in touching {
                self.world.wake_up(body, true);
            }
            self.world.remove_collider(collider);
        }
        receipt
    }

    pub fn insert_body(&mut self, mut body: DynamicsBodyInput) -> Result<(), DynamicsError> {
        if self.bodies.contains_key(&body.id) {
            return Err(DynamicsError::DuplicateBody { body: body.id });
        }
        validate_body(&mut body)?;
        let (builder, collider) = body_builders(&body);
        let (handle, _) = self.world.insert(builder, collider);
        self.bodies.insert(body.id, handle);
        self.shapes.insert(body.id, body.shape);
        Ok(())
    }

    /// Remove a body and every rope and joint attached to it. Returns the
    /// removed rope identities.
    pub fn remove_body(&mut self, id: DynamicsBodyId) -> Vec<u64> {
        let Some(body) = self.bodies.remove(&id) else {
            return Vec::new();
        };
        self.joints.retain(|_, joint| !joint.attaches(id));
        self.detach_soft_bodies_from(body);
        self.shapes.remove(&id);
        self.floating.remove(&id);
        self.kinematic_targets.remove(&id);
        let attached = self.tethers_attached_to(id);
        for tether in &attached {
            if let Some(tether) = self.tethers.remove(tether) {
                tether.remove(&mut self.world);
            }
        }
        self.world.remove_body(body);
        self.contacts
            .retain(|contact| contact.first != id && contact.second != Some(id));
        self.tether_readouts
            .retain(|readout| !attached.contains(&readout.id));
        attached
    }

    /// Replace a body's shape, mass and material in place. Attached ropes
    /// stay attached; `body.translation` and `body.rotation` become its pose.
    pub fn replace_body(&mut self, mut body: DynamicsBodyInput) -> Result<(), DynamicsError> {
        let Some(previous) = self.bodies.get(&body.id).copied() else {
            return Err(DynamicsError::UnknownBody { body: body.id });
        };
        validate_body(&mut body)?;
        let attached = self
            .tethers_attached_to(body.id)
            .into_iter()
            .filter_map(|id| self.tethers.remove(&id))
            .map(|tether| tether.remove(&mut self.world))
            .collect::<Vec<_>>();
        let joints = self
            .joints
            .extract_if(.., |_, joint| joint.attaches(body.id))
            .map(|(_, joint)| joint.remove(&mut self.world))
            .collect::<Vec<_>>();
        let soft_attachments = self.detach_soft_bodies_from(previous);
        self.world.remove_body(previous);
        let (builder, collider) = body_builders(&body);
        let (handle, _) = self.world.insert(builder, collider);
        self.bodies.insert(body.id, handle);
        self.reattach_soft_bodies(body.id, soft_attachments);
        self.shapes.insert(body.id, body.shape);
        for (joint, motor) in joints {
            let (first, second) = (self.bodies[&joint.first], self.bodies[&joint.second]);
            let mut inserted = SolverJoint::insert(joint, &mut self.world, first, second);
            inserted.set_motor(motor, &mut self.world, first, second)?;
            self.joints.insert(joint.id, inserted);
        }
        for (definition, was_taut) in attached {
            let tether = SolverTether::insert(definition, was_taut, &mut self.world, |id| {
                self.bodies.get(&id).copied()
            })?;
            self.tethers.insert(definition.id, tether);
        }
        Ok(())
    }

    /// Teleport a body and set its velocity and sleep state. A non-zero
    /// rotation is normalized.
    pub fn set_body_motion(
        &mut self,
        id: DynamicsBodyId,
        translation: [f64; 3],
        rotation: [f64; 4],
        linear_velocity: [f64; 3],
        angular_velocity: [f64; 3],
        sleeping: bool,
    ) -> Result<(), DynamicsError> {
        let handle = self.handle(id)?;
        let rotation = match unit_rotation(rotation) {
            Some(rotation)
                if translation
                    .into_iter()
                    .chain(linear_velocity)
                    .chain(angular_velocity)
                    .all(f64::is_finite) =>
            {
                rotation
            }
            _ => return Err(DynamicsError::InvalidBody { body: id }),
        };
        let body = &mut self.world.bodies[handle];
        let locks = body.locked_axes();
        body.set_translation(vector(translation), false);
        body.set_rotation(self::rotation(rotation), false);
        body.set_linvel(
            mask_locked_axes(
                linear_velocity,
                [
                    locks.contains(LockedAxes::TRANSLATION_LOCKED_X),
                    locks.contains(LockedAxes::TRANSLATION_LOCKED_Y),
                    locks.contains(LockedAxes::TRANSLATION_LOCKED_Z),
                ],
            ),
            false,
        );
        body.set_angvel(
            mask_locked_axes(
                angular_velocity,
                [
                    locks.contains(LockedAxes::ROTATION_LOCKED_X),
                    locks.contains(LockedAxes::ROTATION_LOCKED_Y),
                    locks.contains(LockedAxes::ROTATION_LOCKED_Z),
                ],
            ),
            false,
        );
        if sleeping {
            body.sleep();
        } else {
            self.world.wake_up(handle, true);
        }
        Ok(())
    }

    pub fn body(&self, id: DynamicsBodyId) -> Option<DynamicsBodyOutput> {
        let body = &self.world.bodies[*self.bodies.get(&id)?];
        Some(DynamicsBodyOutput {
            id,
            translation: body.translation().to_array(),
            rotation: body.rotation().to_array(),
            linear_velocity: body.linvel().to_array(),
            angular_velocity: body.angvel().to_array(),
            sleeping: body.is_sleeping(),
        })
    }

    /// Point, point velocity and impulse response of an enabled body at a
    /// body-local anchor. `None` for an absent or disabled body.
    pub fn observe_anchor(
        &self,
        id: DynamicsBodyId,
        local_anchor: [f64; 3],
    ) -> Option<DynamicsAnchorObservation> {
        let body = &self.world.bodies[*self.bodies.get(&id)?];
        if !body.is_enabled() {
            return None;
        }
        let mut properties = body.mass_properties().clone();
        properties.update_world_mass_properties(RigidBodyType::Dynamic, body.position());
        let point = body.position().transform_point(vector(local_anchor));
        let lever = point - properties.world_com;
        let point_velocity = body.linvel() + body.angvel().cross(lever);
        let response = [Vector::X, Vector::Y, Vector::Z].map(|impulse| {
            let angular = properties.effective_world_inv_inertia * lever.cross(impulse);
            (impulse * properties.effective_inv_mass + angular.cross(lever)).to_array()
        });
        Some(DynamicsAnchorObservation {
            point: point.to_array(),
            point_velocity: point_velocity.to_array(),
            center_of_mass: properties.world_com.to_array(),
            response,
        })
    }

    pub fn rope_solver(&self) -> DynamicsRopeSolverConfig {
        self.rope_solver
    }

    pub fn configure_rope_solver(
        &mut self,
        config: DynamicsRopeSolverConfig,
    ) -> Result<(), DynamicsError> {
        config.validate()?;
        self.rope_solver = config;
        Ok(())
    }

    pub fn tether_count(&self) -> usize {
        self.tethers.len()
    }

    pub fn tether(&self, id: u64) -> Option<DynamicsTether> {
        self.tethers.get(&id).map(|tether| tether.definition)
    }

    /// Create a rope, or edit one. An edit with the same endpoints keeps the
    /// current effective length and reels it toward the new target; different
    /// endpoints make a new attachment, which must be within reach.
    pub fn set_tether(&mut self, definition: DynamicsTether) -> Result<(), DynamicsError> {
        SolverTether::validate(&definition)?;
        if let Some(existing) = self.tethers.get_mut(&definition.id) {
            if existing.definition.first == definition.first
                && existing.definition.second == definition.second
            {
                existing.edit(&mut self.world, definition);
                return Ok(());
            }
        }
        let bodies = &self.bodies;
        let resolve = |id: DynamicsBodyId| bodies.get(&id).copied();
        let distance = SolverTether::endpoint_distance(&definition, &self.world, resolve)?;
        if SolverTether::reach_exceeded(&definition, distance) {
            return Err(DynamicsTetherError::OutOfReach { id: definition.id }.into());
        }
        if let Some(previous) = self.tethers.remove(&definition.id) {
            previous.remove(&mut self.world);
        }
        let tether = SolverTether::insert(definition, false, &mut self.world, resolve)?;
        self.tethers.insert(definition.id, tether);
        self.tether_readouts
            .retain(|readout| readout.id != definition.id);
        Ok(())
    }

    pub fn remove_tether(&mut self, id: u64) -> bool {
        self.tether_readouts.retain(|readout| readout.id != id);
        match self.tethers.remove(&id) {
            Some(tether) => {
                tether.remove(&mut self.world);
                true
            }
            None => false,
        }
    }

    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }

    pub fn joint(&self, id: u64) -> Option<DynamicsJoint> {
        self.joints.get(&id).map(|joint| joint.definition)
    }

    /// Create a limited joint, or replace the one with its identity. The
    /// joint holds the bodies' current placement as their frames say; bodies
    /// that do not meet it are pulled together.
    pub fn set_joint(&mut self, definition: DynamicsJoint) -> Result<(), DynamicsError> {
        SolverJoint::validate(&definition)?;
        let resolve = |body: DynamicsBodyId| {
            self.bodies
                .get(&body)
                .copied()
                .ok_or(DynamicsJointError::UnknownBody {
                    id: definition.id,
                    body,
                })
        };
        let (first, second) = (resolve(definition.first)?, resolve(definition.second)?);
        if let Some(previous) = self.joints.remove(&definition.id) {
            previous.remove(&mut self.world);
        }
        self.joints.insert(
            definition.id,
            SolverJoint::insert(definition, &mut self.world, first, second),
        );
        Ok(())
    }

    /// Drive joint `id` with `motor`, or stop driving it with `None`.
    /// Replacing the joint with `set_joint` stops its motor.
    pub fn set_joint_motor(
        &mut self,
        id: u64,
        motor: Option<DynamicsJointMotor>,
    ) -> Result<(), DynamicsError> {
        let joint = self
            .joints
            .get_mut(&id)
            .ok_or(DynamicsJointError::Invalid { id })?;
        let (first, second) = (
            self.bodies[&joint.definition.first],
            self.bodies[&joint.definition.second],
        );
        joint.set_motor(motor, &mut self.world, first, second)?;
        Ok(())
    }

    pub fn joint_motor(&self, id: u64) -> Option<DynamicsJointMotor> {
        self.joints.get(&id)?.motor()
    }

    /// Move a body kinematically to `placement` over the next step, at an
    /// even pace through all of its substeps, or make it simulated again with
    /// `None`, keeping the velocity it moved with. A kinematic body given no
    /// placement for a step stays where it is.
    pub fn set_body_kinematic(
        &mut self,
        id: DynamicsBodyId,
        placement: Option<([f64; 3], [f64; 4])>,
    ) -> Result<(), DynamicsError> {
        let handle = self.handle(id)?;
        let body = &mut self.world.bodies[handle];
        match placement {
            Some((translation, rotation)) => {
                let rotation = unit_rotation(rotation)
                    .filter(|_| translation.into_iter().all(f64::is_finite))
                    .ok_or(DynamicsError::InvalidBody { body: id })?;
                if body.body_type() != RigidBodyType::KinematicPositionBased {
                    body.set_body_type(RigidBodyType::KinematicPositionBased, true);
                }
                self.kinematic_targets.insert(
                    id,
                    Pose::from_parts(vector(translation), self::rotation(rotation)),
                );
            }
            None => {
                if body.body_type() != RigidBodyType::Dynamic {
                    body.set_body_type(RigidBodyType::Dynamic, true);
                }
                self.kinematic_targets.remove(&id);
            }
        }
        Ok(())
    }

    pub fn body_is_kinematic(&self, id: DynamicsBodyId) -> bool {
        self.bodies
            .get(&id)
            .is_some_and(|handle| self.world.bodies[*handle].is_kinematic())
    }

    pub fn remove_joint(&mut self, id: u64) -> bool {
        match self.joints.remove(&id) {
            Some(joint) => {
                joint.remove(&mut self.world);
                true
            }
            None => false,
        }
    }

    /// Apply `impulse` at the world `point` of a body, waking it.
    pub fn apply_impulse_at_point(
        &mut self,
        id: DynamicsBodyId,
        point: [f64; 3],
        impulse: [f64; 3],
    ) -> Result<(), DynamicsError> {
        let handle = self.handle(id)?;
        if !point.into_iter().chain(impulse).all(f64::is_finite) {
            return Err(DynamicsError::InvalidAction { body: id });
        }
        self.world.bodies[handle].apply_impulse_at_point(vector(impulse), vector(point), true);
        Ok(())
    }

    /// Move every body and fixed rope anchor by `delta`, for a world-origin
    /// rebase. Velocities, contacts and rope state are unchanged.
    pub fn translate(&mut self, delta: [f64; 3]) {
        let delta = vector(delta);
        for handle in self.bodies.values() {
            let body = &mut self.world.bodies[*handle];
            let translation = body.translation() + delta;
            body.set_translation(translation, false);
        }
        for tether in self.tethers.values_mut() {
            tether.translate(&mut self.world, delta);
        }
        self.translate_soft_bodies(delta);
        for readout in &mut self.tether_readouts {
            readout.first = (vector(readout.first) + delta).to_array();
            readout.second = (vector(readout.second) + delta).to_array();
        }
    }

    /// Replace the world's water and the bodies it acts on. A body takes its
    /// own plane ([`Self::set_body_water`]) if it has one, else the first
    /// plane whose rectangle holds its centre; out of every plane, or above
    /// its surface, nothing acts on it.
    pub fn set_water(
        &mut self,
        planes: Vec<DynamicsWaterPlane>,
        bodies: impl IntoIterator<Item = DynamicsBodyId>,
    ) -> Result<(), DynamicsError> {
        if let Some(plane) = planes.iter().position(|plane| !plane.is_valid()) {
            return Err(DynamicsError::InvalidWater { plane });
        }
        let mut floating = BTreeMap::new();
        for body in bodies {
            self.handle(body)?;
            let readout = self
                .floating
                .get(&body)
                .map(|f| f.readout)
                .unwrap_or_default();
            floating.insert(
                body,
                Floating {
                    readout,
                    local: None,
                },
            );
        }
        self.water = planes;
        self.floating = floating;
        Ok(())
    }

    /// Give a floating body its own plane for the following steps, wherever
    /// it moves (the water a field reports under it), or clear it.
    pub fn set_body_water(
        &mut self,
        id: DynamicsBodyId,
        plane: Option<DynamicsWaterPlane>,
    ) -> Result<(), DynamicsError> {
        if plane.is_some_and(|plane| !plane.is_valid()) {
            return Err(DynamicsError::InvalidWater { plane: 0 });
        }
        let floating = self
            .floating
            .get_mut(&id)
            .ok_or(DynamicsError::UnknownBody { body: id })?;
        floating.local = plane;
        Ok(())
    }

    /// What the water did to a floating body over the latest step.
    pub fn water_readout(&self, id: DynamicsBodyId) -> Option<DynamicsWaterReadout> {
        self.floating.get(&id).map(|floating| floating.readout)
    }

    /// One substep of water on every floating body: buoyancy as an impulse at
    /// the submerged centroid, drag as the velocity change toward the current.
    /// Adds to `sums`, one entry per floating body in order.
    fn apply_water(&mut self, dt: f64, sums: &mut [DynamicsWaterReadout]) {
        let gravity = self.world.gravity;
        let weight = gravity.length();
        let up = if weight > 0.0 {
            -gravity / weight
        } else {
            Vector::Y
        };
        for ((id, floating), sum) in self.floating.iter_mut().zip(sums.iter_mut()) {
            let last = &mut floating.readout;
            let (Some(&handle), Some(&shape)) = (self.bodies.get(id), self.shapes.get(id)) else {
                continue;
            };
            let body = &mut self.world.bodies[handle];
            if !body.is_enabled() {
                continue;
            }
            let centre = body.translation();
            let local = floating.local.as_ref();
            let Some(plane) = local.or_else(|| self.water.iter().find(|plane| plane.holds(centre)))
            else {
                last.submerged_fraction = 0.0;
                continue;
            };
            let part = submerged(shape, centre, *body.rotation(), plane.surface);
            let fraction = (part.volume / shape_volume(shape)).min(1.0);
            last.submerged_fraction = fraction;
            if fraction <= 0.0 {
                continue;
            }
            let buoyancy = up * (plane.density * weight * part.volume);
            let com = body.center_of_mass();
            body.apply_impulse_at_point(buoyancy * dt, part.centroid, true);
            let current = Vector::new(plane.current[0], 0.0, plane.current[1]);
            let relative = body.linvel() - current;
            let change = relative * ((-plane.linear_drag * fraction * dt).exp() - 1.0);
            let drag = change * (body.mass() / dt);
            body.apply_impulse(drag * dt, true);
            let spin = body.angvel();
            let slowed = spin * (-plane.angular_drag * fraction * dt).exp();
            body.set_angvel(slowed, true);
            // The torque that slowing took: the change in angular momentum,
            // through the body's inertia in its principal frame.
            let mass = &body.mass_properties().local_mprops;
            let frame = *body.rotation() * mass.principal_inertia_local_frame;
            let change = frame.inverse() * (slowed - spin);
            let spin_drag = frame * (change * mass.principal_inertia()) / dt;
            sum.buoyancy = add(sum.buoyancy, buoyancy.to_array());
            sum.drag = add(sum.drag, drag.to_array());
            let torque = (part.centroid - com).cross(buoyancy) + spin_drag;
            sum.torque = add(sum.torque, torque.to_array());
        }
    }

    pub fn step(
        &mut self,
        step_seconds: f64,
        steps: u32,
        actions: &[DynamicsAction],
    ) -> Result<DynamicsStepReceipt, DynamicsError> {
        if !step_seconds.is_finite() || step_seconds <= 0.0 {
            return Err(DynamicsError::InvalidStep {
                actual: step_seconds,
            });
        }
        let handles = actions
            .iter()
            .map(|action| {
                let handle = self.handle(action.body)?;
                if !action.is_finite() {
                    return Err(DynamicsError::InvalidAction { body: action.body });
                }
                Ok(handle)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut forced = Vec::new();
        for (action, handle) in actions.iter().zip(handles) {
            let body = &mut self.world.bodies[handle];
            body.apply_impulse(vector(action.impulse), action.wake);
            body.apply_torque_impulse(vector(action.torque_impulse), action.wake);
            if action.force != [0.0; 3] || action.torque != [0.0; 3] {
                body.add_force(vector(action.force), action.wake);
                body.add_torque(vector(action.torque), action.wake);
                forced.push(handle);
            }
            if action.wake {
                self.world.wake_up(handle, true);
            }
        }

        // Ropes and limited joints need the finer solve; soft bodies take
        // their own substeps.
        let roped = !self.tethers.is_empty() || !self.joints.is_empty();
        let soft = !self.soft_bodies.is_empty();
        let subdivisions = match (roped, soft) {
            (true, true) => self
                .rope_solver
                .substeps
                .max(self.soft_settings.substeps as usize),
            (true, false) => self.rope_solver.substeps,
            (false, true) => self.soft_settings.substeps as usize,
            (false, false) => 1,
        };
        let dt = step_seconds / subdivisions as f64;
        self.world.integration_parameters = IntegrationParameters {
            dt,
            num_solver_iterations: if roped {
                self.rope_solver.iterations
            } else {
                IntegrationParameters::default().num_solver_iterations
            },
            soft_bodies: self.soft_settings.rapier(),
            ..IntegrationParameters::default()
        };
        self.begin_soft_step();
        for tether in self.tethers.values_mut() {
            tether.begin_step();
        }
        let substeps = steps as usize * subdivisions;
        let mut water = vec![DynamicsWaterReadout::default(); self.floating.len()];
        // Kinematic bodies reach their targets at the end of the step, at the
        // step's velocity in every substep rather than all in the first.
        let moves = std::mem::take(&mut self.kinematic_targets)
            .into_iter()
            .filter_map(|(id, target)| {
                let handle = *self.bodies.get(&id)?;
                Some((handle, *self.world.bodies[handle].position(), target))
            })
            .collect::<Vec<_>>();
        for substep in 0..substeps {
            let along = (substep + 1) as f64 / substeps as f64;
            for (handle, start, target) in &moves {
                self.world.bodies[*handle].set_next_kinematic_position(start.lerp(target, along));
            }
            for tether in self.tethers.values_mut() {
                tether.reel(&mut self.world, dt);
            }
            if !self.floating.is_empty() {
                self.apply_water(dt, &mut water);
            }
            if soft {
                self.soft_substep(along);
                let tears = TearEvents::default();
                self.world.step_with_events(&(), &tears);
                self.reconcile_soft_tears(tears.take());
            } else {
                self.world.step();
            }
            self.resist_rolling(dt);
            for tether in self.tethers.values_mut() {
                tether.accumulate(&self.world);
            }
        }
        let average = |value: [f64; 3]| value.map(|component| component / substeps.max(1) as f64);
        for (floating, sum) in self.floating.values_mut().zip(water) {
            let last = &mut floating.readout;
            last.buoyancy = average(sum.buoyancy);
            last.drag = average(sum.drag);
            last.torque = average(sum.torque);
        }
        // Rapier keeps user forces until reset; an action's force lasts one call.
        for handle in forced {
            let body = &mut self.world.bodies[handle];
            body.reset_forces(false);
            body.reset_torques(false);
        }

        if soft {
            self.end_soft_step();
        }
        self.contacts.clear();
        for pair in self
            .world
            .contact_pairs()
            .filter(|pair| pair.has_any_active_contact())
        {
            let (first, second) = (
                self.world.colliders[pair.collider1].user_data,
                self.world.colliders[pair.collider2].user_data,
            );
            // Soft-body contacts are counted on the soft body.
            if soft_id(first).is_some() || soft_id(second).is_some() {
                continue;
            }
            let first = dynamic_id(first);
            let second = dynamic_id(second);
            let (first, second) = match (first, second) {
                (Some(first), second) => (first, second),
                (None, Some(second)) => (second, None),
                (None, None) => continue,
            };
            self.contacts.push(DynamicsContact {
                first,
                second,
                impulse: pair.total_impulse().to_array(),
                impulse_magnitude: pair.total_impulse_magnitude(),
            });
        }
        self.contacts
            .sort_by_key(|contact| (contact.first, contact.second));
        self.tether_readouts = self
            .tethers
            .values_mut()
            .map(|tether| tether.readout(&self.world))
            .collect();
        self.generation += 1;
        Ok(DynamicsStepReceipt {
            generation: self.generation,
            body_count: self.bodies.len(),
            contact_count: self.contacts.len(),
            rope_link_count: self.tethers.len(),
            rope_substeps: if roped { self.rope_solver.substeps } else { 0 },
            rope_iterations: if roped {
                self.rope_solver.iterations
            } else {
                0
            },
        })
    }

    /// Rolling resistance, which Rapier has none of: a sphere or capsule
    /// touching anything loses rolling speed (its spin about axes along the
    /// contact) at `ROLLING_RESISTANCE` times gravity, as on hard ground.
    /// Without it a capsule lying on its side keeps rolling on the solver's
    /// contact corrections and never sleeps (#10203).
    fn resist_rolling(&mut self, dt: f64) {
        let deceleration = ROLLING_RESISTANCE * self.world.gravity.length();
        if deceleration <= 0.0 {
            return;
        }
        let mut touching: BTreeMap<DynamicsBodyId, Vector> = BTreeMap::new();
        for pair in self
            .world
            .contact_pairs()
            .filter(|pair| pair.has_any_active_contact())
        {
            let Some(manifold) = pair
                .solver_manifolds()
                .iter()
                .find(|manifold| !manifold.data.solver_contacts.is_empty())
            else {
                continue;
            };
            for collider in [pair.collider1, pair.collider2] {
                if let Some(id) = dynamic_id(self.world.colliders[collider].user_data) {
                    touching.entry(id).or_insert(manifold.data.normal);
                }
            }
        }
        for (id, normal) in touching {
            let radius = match self.shapes.get(&id) {
                Some(DynamicsShape::Sphere { radius }) => *radius,
                Some(DynamicsShape::CapsuleY { radius, .. }) => *radius,
                _ => continue,
            };
            let Some(&handle) = self.bodies.get(&id) else {
                continue;
            };
            let body = &mut self.world.bodies[handle];
            if body.is_sleeping() {
                continue;
            }
            let Some(normal) = normal.try_normalize() else {
                continue;
            };
            let locks = body.locked_axes();
            let locked = [
                locks.contains(LockedAxes::ROTATION_LOCKED_X),
                locks.contains(LockedAxes::ROTATION_LOCKED_Y),
                locks.contains(LockedAxes::ROTATION_LOCKED_Z),
            ];
            let spin = body.angvel();
            let resisted = resisted_spin(spin, normal, deceleration * dt / radius, locked);
            if resisted != spin {
                body.set_angvel(resisted, false);
            }
        }
    }

    pub(crate) fn handle(&self, id: DynamicsBodyId) -> Result<RigidBodyHandle, DynamicsError> {
        self.bodies
            .get(&id)
            .copied()
            .ok_or(DynamicsError::UnknownBody { body: id })
    }

    fn tethers_attached_to(&self, id: DynamicsBodyId) -> Vec<u64> {
        self.tethers
            .values()
            .filter(|tether| {
                tether.definition.first.body() == Some(id)
                    || tether.definition.second.body() == Some(id)
            })
            .map(|tether| tether.definition.id)
            .collect()
    }
}

fn body_builders(body: &DynamicsBodyInput) -> (RigidBodyBuilder, ColliderBuilder) {
    let builder = RigidBodyBuilder::dynamic()
        .translation(vector(body.translation))
        .rotation(vector3(body.rotation))
        // A locked axis cannot carry initial velocity.
        .linvel(mask_locked_axes(
            body.linear_velocity,
            body.locked_translation_axes,
        ))
        .angvel(mask_locked_axes(
            body.angular_velocity,
            body.locked_rotation_axes,
        ))
        .enabled_translations(
            !body.locked_translation_axes[0],
            !body.locked_translation_axes[1],
            !body.locked_translation_axes[2],
        )
        .enabled_rotations(
            !body.locked_rotation_axes[0],
            !body.locked_rotation_axes[1],
            !body.locked_rotation_axes[2],
        )
        .linear_damping(body.linear_damping)
        .angular_damping(body.angular_damping)
        .gravity_scale(body.gravity_scale)
        .ccd_enabled(body.continuous_collision)
        .sleeping(body.sleeping)
        .enabled(body.enabled)
        .user_data(u128::from(body.id.0) + 1);
    let collider = ColliderBuilder::new(shared_shape(body.shape));
    let collider = if let Some(properties) = body.mass_properties {
        collider.mass_properties(MassProperties::with_principal_inertia_frame(
            vector(properties.center_of_mass),
            body.mass,
            vector(properties.principal_inertia),
            rotation(properties.principal_inertia_local_frame),
        ))
    } else {
        collider.mass(body.mass)
    }
    .friction(body.friction)
    .restitution(body.restitution)
    .collision_groups(InteractionGroups::new(
        Group::from_bits_retain(body.collision_groups),
        Group::from_bits_retain(body.collision_mask),
        InteractionTestMode::And,
    ))
    .user_data(u128::from(body.id.0) + 1);
    (builder, collider)
}

/// Refuses what Rapier cannot hold, and normalizes a non-zero rotation.
fn validate_body(body: &mut DynamicsBodyInput) -> Result<(), DynamicsError> {
    let finite = body
        .translation
        .into_iter()
        .chain(body.linear_velocity)
        .chain(body.angular_velocity)
        .chain([
            body.mass,
            body.linear_damping,
            body.angular_damping,
            body.gravity_scale,
            body.friction,
            body.restitution,
        ])
        .all(f64::is_finite);
    let shape_valid = match body.shape {
        DynamicsShape::Sphere { radius } => positive(radius),
        DynamicsShape::Cuboid { half_extents } => half_extents.into_iter().all(positive),
        DynamicsShape::CapsuleY {
            half_height,
            radius,
        } => positive(half_height) && positive(radius),
    };
    match unit_rotation(body.rotation) {
        Some(rotation)
            if finite
                && shape_valid
                && positive(body.mass)
                && body.mass_properties.is_none_or(valid_mass_properties) =>
        {
            body.rotation = rotation;
            Ok(())
        }
        _ => Err(DynamicsError::InvalidBody { body: body.id }),
    }
}

fn valid_mass_properties(properties: DynamicsMassProperties) -> bool {
    properties.center_of_mass.into_iter().all(f64::is_finite)
        && properties.principal_inertia.into_iter().all(positive)
        && properties
            .principal_inertia_local_frame
            .into_iter()
            .all(f64::is_finite)
        && (norm_squared(properties.principal_inertia_local_frame) - 1.0).abs()
            <= MASS_PROPERTIES_FRAME_NORMALIZATION_TOLERANCE
}

/// The unit quaternion of a finite, non-zero one. Rapier and glam turn a
/// zero or non-finite rotation into NaN poses.
pub(crate) fn unit_rotation(value: [f64; 4]) -> Option<[f64; 4]> {
    let length = norm_squared(value).sqrt();
    (length.is_finite() && length > 0.0).then(|| value.map(|component| component / length))
}

fn norm_squared(value: [f64; 4]) -> f64 {
    value.into_iter().map(|value| value * value).sum()
}

/// `spin` with its rolling part (about axes along a contact of unit `normal`)
/// slowed by `loss` rad/s, and with the body's `locked` world rotation axes
/// kept still: on a sloped contact, slowing only the rolling part would
/// otherwise leave a component along a locked axis.
fn resisted_spin(spin: Vector, normal: Vector, loss: f64, locked: [bool; 3]) -> Vector {
    let about_normal = normal * spin.dot(normal);
    let rolling = spin - about_normal;
    let speed = rolling.length();
    if speed <= 0.0 {
        return spin;
    }
    let slowed = about_normal + rolling * ((speed - loss).max(0.0) / speed);
    mask_locked_axes(slowed.to_array(), locked)
}

fn mask_locked_axes(value: [f64; 3], locked_axes: [bool; 3]) -> Vector {
    Vector::new(
        if locked_axes[0] { 0.0 } else { value[0] },
        if locked_axes[1] { 0.0 } else { value[1] },
        if locked_axes[2] { 0.0 } else { value[2] },
    )
}

fn shape_key(shape: &SharedShape) -> usize {
    Arc::as_ptr(&shape.0).cast::<()>() as usize
}

fn shared_shape(shape: DynamicsShape) -> SharedShape {
    match shape {
        DynamicsShape::Sphere { radius } => SharedShape::ball(radius),
        DynamicsShape::Cuboid { half_extents } => {
            SharedShape::cuboid(half_extents[0], half_extents[1], half_extents[2])
        }
        DynamicsShape::CapsuleY {
            half_height,
            radius,
        } => SharedShape::capsule_y(half_height, radius),
    }
}

/// A body the water acts on.
#[derive(Debug, Clone, Copy, Default)]
struct Floating {
    readout: DynamicsWaterReadout,
    /// Its own plane, ahead of the world's.
    local: Option<DynamicsWaterPlane>,
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| a[axis] + b[axis])
}

fn vector(value: [f64; 3]) -> Vector {
    Vector::new(value[0], value[1], value[2])
}

fn vector3(value: [f64; 4]) -> Vector {
    let rotation = rotation(value);
    rotation.to_scaled_axis()
}

fn rotation(value: [f64; 4]) -> Rotation {
    Rotation::from_xyzw(value[0], value[1], value[2], value[3])
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn dynamic_id(user_data: u128) -> Option<DynamicsBodyId> {
    (user_data != 0 && soft_id(user_data).is_none()).then(|| DynamicsBodyId((user_data - 1) as u64))
}

impl CollisionProjection {
    fn dynamics_shapes(&self) -> Vec<SharedShape> {
        let mut shapes =
            Vec::with_capacity(self.chunks.len() + self.static_meshes.instance_count());
        for chunk in self.chunks.values() {
            if let Some(cubes) = &chunk.cubes {
                shapes.push(SharedShape(cubes.clone()));
            }
            if let Some(surface) = &chunk.surface {
                shapes.push(SharedShape(surface.shape.clone()));
            }
        }
        shapes.extend(self.static_meshes.dynamics_shapes());
        shapes
    }
}

#[cfg(test)]
mod tests {
    use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
    use core_voxel::VoxelValue;
    use svc_spatial::VoxelWorld;
    use svc_volume::VoxelChunk;

    use super::*;
    use crate::DynamicsTetherEndpoint;

    const TICK: f64 = 1.0 / 60.0;
    const GRAVITY: [f64; 3] = [0.0, -9.81, 0.0];

    fn body() -> DynamicsBodyInput {
        DynamicsBodyInput {
            id: DynamicsBodyId(1),
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            shape: DynamicsShape::Sphere { radius: 0.5 },
            mass: 1.0,
            mass_properties: None,
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            locked_translation_axes: [false; 3],
            locked_rotation_axes: [false; 3],
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 0.0,
            friction: 0.5,
            restitution: 0.0,
            collision_groups: u32::MAX,
            collision_mask: u32::MAX,
            enabled: true,
            sleeping: false,
            continuous_collision: false,
        }
    }

    fn explicit_body() -> DynamicsBodyInput {
        let mut body = body();
        body.mass = 2.0;
        body.mass_properties = Some(DynamicsMassProperties {
            center_of_mass: [0.2, -0.1, 0.0],
            principal_inertia: [1.0, 2.0, 4.0],
            principal_inertia_local_frame: [0.0, 0.0, 0.0, 1.0],
        });
        body
    }

    fn tether() -> DynamicsTether {
        DynamicsTether {
            id: 7,
            first: DynamicsTetherEndpoint::Fixed([0.0; 3]),
            second: DynamicsTetherEndpoint::Body {
                body: DynamicsBodyId(1),
                local_anchor: [0.0; 3],
            },
            maximum_length: 3.0,
            target_length: 3.0,
            reel_speed: 0.25,
            contacts_enabled: true,
        }
    }

    fn solver(gravity: [f64; 3], bodies: &[DynamicsBodyInput]) -> DynamicsSolver {
        let mut solver = DynamicsSolver::new(gravity);
        for body in bodies {
            solver.insert_body(*body).unwrap();
        }
        solver
    }

    fn output(solver: &DynamicsSolver, id: u64) -> DynamicsBodyOutput {
        solver.body(DynamicsBodyId(id)).unwrap()
    }

    fn spec() -> VoxelGridSpec {
        VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(8).unwrap()).unwrap()
    }

    /// A one-voxel-thick floor at y = -1..0 across each listed chunk column.
    fn floor_world(columns: &[i64]) -> VoxelWorld {
        let mut world = VoxelWorld::new(spec());
        for &column in columns {
            let mut chunk = VoxelChunk::from_spec(&spec());
            for x in 0..8 {
                for z in 0..8 {
                    chunk
                        .set(LocalVoxelCoord::new(x, 7, z), VoxelValue::solid_raw(1))
                        .unwrap();
                }
            }
            world.insert(ChunkCoord::new(column, -1, 0), chunk);
        }
        world.drain_dirty();
        world
    }

    fn settle(solver: &mut DynamicsSolver, ticks: usize) {
        for _ in 0..ticks {
            solver.step(TICK, 1, &[]).unwrap();
        }
    }

    #[test]
    fn anchor_response_matches_off_center_impulse_with_rotated_inertia_and_locks() {
        for locked in [false, true] {
            let mut body = explicit_body();
            body.rotation = [0.0, 0.0, (0.3_f64).sin(), (0.3_f64).cos()];
            body.locked_translation_axes = [locked, false, false];
            body.locked_rotation_axes = [false, locked, false];
            let mut solver = solver([0.0; 3], &[body]);
            let anchor = solver.observe_anchor(body.id, [0.7, 0.4, -0.3]).unwrap();
            let lever = vector(anchor.point) - vector(anchor.center_of_mass);
            let impulse = Vector::new(0.2, -0.1, 0.3);
            let expected = vector(anchor.response[0]) * impulse.x
                + vector(anchor.response[1]) * impulse.y
                + vector(anchor.response[2]) * impulse.z;
            let mut action = DynamicsAction::impulse(body.id, impulse.to_array());
            action.torque_impulse = lever.cross(impulse).to_array();
            solver.step(TICK, 1, &[action]).unwrap();
            let after = output(&solver, 1);
            let actual =
                vector(after.linear_velocity) + vector(after.angular_velocity).cross(lever);
            assert!(
                (actual - expected).length() < 0.0001,
                "{actual:?} vs {expected:?}"
            );
        }
    }

    #[test]
    fn hanging_mass_keeps_one_catch_and_reports_its_weight() {
        let mut body = body();
        body.translation = [0.0, -3.0, 0.0];
        body.gravity_scale = 1.0;
        let mut solver = solver(GRAVITY, &[body]);
        solver.set_tether(tether()).unwrap();
        for tick in 0..60 {
            solver.step(TICK, 1, &[]).unwrap();
            let readout = solver.tether_readouts()[0];
            assert!(readout.taut);
            assert_eq!(readout.caught, tick == 0);
            assert!((readout.distance - 3.0).abs() < 0.001);
            assert!((readout.force_proxy - 9.81).abs() < 0.005, "{readout:?}");
        }
    }

    #[test]
    fn two_roped_bodies_exchange_momentum_deterministically() {
        let mut first = body();
        first.translation = [-1.5, 0.0, 0.0];
        first.linear_velocity = [-3.0, 0.0, 0.0];
        let mut second = first;
        second.id = DynamicsBodyId(2);
        second.translation = [1.5, 0.0, 0.0];
        second.linear_velocity = [3.0, 0.0, 0.0];
        let mut rope = tether();
        rope.first = DynamicsTetherEndpoint::Body {
            body: first.id,
            local_anchor: [0.5, 0.0, 0.0],
        };
        rope.second = DynamicsTetherEndpoint::Body {
            body: second.id,
            local_anchor: [-0.5, 0.0, 0.0],
        };
        rope.maximum_length = 2.0;
        rope.target_length = 2.0;
        let run = || {
            let mut solver = solver([0.0; 3], &[first, second]);
            solver.set_tether(rope).unwrap();
            solver.step(TICK, 1, &[]).unwrap();
            (
                output(&solver, 1),
                output(&solver, 2),
                solver.tether_readouts()[0],
            )
        };
        let (a, b, readout) = run();
        assert_eq!((a, b, readout), run());
        assert!((a.linear_velocity[0] + b.linear_velocity[0]).abs() < 1e-9);
        assert!((readout.first[0] - a.translation[0] - 0.5).abs() < 1e-9);
        assert!(readout.distance < 2.01);
        assert!([a, b]
            .iter()
            .all(|body| body.linear_velocity[0].abs() < 0.01));
    }

    #[test]
    fn slack_catch_swing_and_release_do_not_create_energy() {
        let mut body = body();
        body.translation = [1.0, -1.0, 0.0];
        body.linear_velocity = [8.0, -15.0, 0.0];
        body.gravity_scale = 1.0;
        body.continuous_collision = true;
        let initial_energy =
            0.5 * vector(body.linear_velocity).length_squared() + 9.81 * body.translation[1];
        let mut solver = solver(GRAVITY, &[body]);
        solver.set_tether(tether()).unwrap();
        let mut caught = false;
        let mut crossed = false;
        for _ in 0..600 {
            solver.step(TICK, 1, &[]).unwrap();
            let after = output(&solver, 1);
            let energy =
                0.5 * vector(after.linear_velocity).length_squared() + 9.81 * after.translation[1];
            assert!(energy <= initial_energy + 0.01, "energy grew: {energy}");
            let readout = solver.tether_readouts()[0];
            assert!(readout.distance < 3.01);
            caught |= readout.caught;
            crossed |= after.translation[0] < -0.5;
        }
        assert!(caught && crossed);
        assert!(solver.remove_tether(7));
        let before = output(&solver, 1);
        solver.step(TICK, 1, &[]).unwrap();
        let released = output(&solver, 1);
        let expected = before.linear_velocity[1] - 9.81 * TICK;
        assert!((released.linear_velocity[1] - expected).abs() < 1e-9);
    }

    #[test]
    fn off_center_rope_rotates_body_and_unequal_masses_conserve_momentum() {
        let mut first = body();
        first.translation = [-1.0, 0.0, 0.0];
        first.linear_velocity = [-2.0, 0.0, 0.0];
        let mut second = first;
        second.id = DynamicsBodyId(2);
        second.mass = 4.0;
        second.translation = [1.0, 0.0, 0.0];
        second.linear_velocity = [0.5, 0.0, 0.0];
        let mut rope = tether();
        rope.first = DynamicsTetherEndpoint::Body {
            body: first.id,
            local_anchor: [0.0, 0.5, 0.0],
        };
        rope.second = DynamicsTetherEndpoint::Body {
            body: second.id,
            local_anchor: [0.0, 0.5, 0.0],
        };
        rope.maximum_length = 2.0;
        rope.target_length = 2.0;
        let mut solver = solver([0.0; 3], &[first, second]);
        solver.set_tether(rope).unwrap();
        solver.step(TICK, 1, &[]).unwrap();
        let (a, b) = (output(&solver, 1), output(&solver, 2));
        let readout = solver.tether_readouts()[0];
        assert!(a.angular_velocity[2].abs() > 0.01);
        assert!((a.linear_velocity[0] + 4.0 * b.linear_velocity[0]).abs() < 1e-8);
        assert!(readout.distance < 2.001);
        let anchor = vector(a.translation) + rotation(a.rotation) * Vector::new(0.0, 0.5, 0.0);
        assert!((anchor - vector(readout.first)).length() < 1e-9);
    }

    #[test]
    fn reeling_moves_effective_length_at_the_rope_speed_and_edits_keep_it() {
        let mut body = body();
        body.translation = [0.0, -3.0, 0.0];
        let mut solver = solver([0.0; 3], &[body]);
        let mut rope = tether();
        rope.target_length = 1.0;
        solver.set_tether(rope).unwrap();
        solver.step(TICK, 1, &[]).unwrap();
        let reeled = 3.0 - 0.25 * TICK;
        assert!((solver.tether_readouts()[0].maximum_length - reeled).abs() < 1e-9);
        assert!(output(&solver, 1).linear_velocity[1] < 0.3);
        // Re-authoring the same attachment changes the target, not the length.
        rope.maximum_length = 3.0;
        rope.target_length = 2.0;
        solver.set_tether(rope).unwrap();
        let current = solver.tether(7).unwrap();
        assert!((current.maximum_length - reeled).abs() < 1e-9);
        assert_eq!(current.target_length, 2.0);
    }

    #[test]
    fn new_attachments_need_a_known_body_and_reach() {
        let mut solver = solver([0.0; 3], &[body()]);
        let mut rope = tether();
        rope.second = DynamicsTetherEndpoint::Body {
            body: DynamicsBodyId(99),
            local_anchor: [0.0; 3],
        };
        assert_eq!(
            solver.set_tether(rope),
            Err(DynamicsTetherError::InvalidAnchor { id: 7 }.into())
        );
        let mut rope = tether();
        rope.first = DynamicsTetherEndpoint::Fixed([10.0, 0.0, 0.0]);
        assert_eq!(
            solver.set_tether(rope),
            Err(DynamicsTetherError::OutOfReach { id: 7 }.into())
        );
        assert_eq!(solver.tether_count(), 0);
    }

    #[test]
    fn explicit_mass_properties_use_authored_total_once_and_preserve_asymmetry() {
        let mut solver = solver([0.0; 3], &[explicit_body()]);
        solver
            .step(
                TICK,
                1,
                &[DynamicsAction {
                    body: DynamicsBodyId(1),
                    force: [2.0, 0.0, 0.0],
                    torque: [0.0, 1.0, 0.0],
                    impulse: [0.0; 3],
                    torque_impulse: [0.0; 3],
                    wake: true,
                }],
            )
            .unwrap();
        let body = output(&solver, 1);
        // Force / authored total mass, not force / (collider mass + authored mass).
        assert!((body.linear_velocity[0] - (1.0 / 60.0)).abs() < 1.0e-6);
        // The Y principal inertia is 2, so unit torque gives half the angular
        // velocity of a unit inertia axis.
        assert!((body.angular_velocity[1] - (1.0 / 120.0)).abs() < 1.0e-6);
    }

    #[test]
    fn invalid_mass_properties_and_nonfinite_actions_are_refused() {
        let mut solver = DynamicsSolver::new([0.0; 3]);
        let mut body = explicit_body();
        body.mass_properties.as_mut().unwrap().principal_inertia[0] = 0.0;
        assert_eq!(
            solver.insert_body(body),
            Err(DynamicsError::InvalidBody {
                body: DynamicsBodyId(1)
            })
        );
        solver.insert_body(explicit_body()).unwrap();
        let mut action = DynamicsAction::impulse(DynamicsBodyId(1), [1.0, 0.0, 0.0]);
        action.force[0] = f64::NAN;
        let good = DynamicsAction::impulse(DynamicsBodyId(1), [1.0, 0.0, 0.0]);
        assert_eq!(
            solver.step(TICK, 1, &[good, action]),
            Err(DynamicsError::InvalidAction {
                body: DynamicsBodyId(1)
            })
        );
        // Nothing from the refused call was applied.
        assert_eq!(output(&solver, 1).linear_velocity, [0.0; 3]);
        let unknown = DynamicsAction::impulse(DynamicsBodyId(9), [1.0, 0.0, 0.0]);
        assert_eq!(
            solver.step(TICK, 1, &[good, unknown]),
            Err(DynamicsError::UnknownBody {
                body: DynamicsBodyId(9)
            })
        );
        assert_eq!(output(&solver, 1).linear_velocity, [0.0; 3]);
    }

    #[test]
    fn a_non_zero_rotation_is_normalized_and_a_zero_one_refused() {
        let unit = |rotation: [f64; 4]| (norm_squared(rotation) - 1.0).abs() < 1.0e-12;
        let mut tilted = body();
        tilted.rotation = [0.0, 0.0, 0.0, 1.001];
        let mut solver = solver([0.0; 3], &[tilted]);
        let created = output(&solver, 1).rotation;
        assert!(unit(created), "{created:?}");
        assert_eq!(created, [0.0, 0.0, 0.0, 1.0]);

        let id = DynamicsBodyId(1);
        solver
            .set_body_motion(
                id,
                [0.0; 3],
                [0.0, 0.6, 0.0, 0.9],
                [0.0; 3],
                [0.0; 3],
                false,
            )
            .unwrap();
        let moved = output(&solver, 1).rotation;
        assert!(unit(moved), "{moved:?}");
        assert!(
            (moved[1] / moved[3] - 0.6 / 0.9).abs() < 1.0e-9,
            "{moved:?}"
        );

        // A zero or overflowing quaternion has no unit; Rapier would hold NaN.
        for rotation in [[0.0; 4], [0.0, 0.0, 0.0, f64::MAX]] {
            assert_eq!(
                solver.set_body_motion(id, [0.0; 3], rotation, [0.0; 3], [0.0; 3], false),
                Err(DynamicsError::InvalidBody { body: id })
            );
            let mut refused = body();
            refused.id = DynamicsBodyId(2);
            refused.rotation = rotation;
            assert_eq!(
                solver.insert_body(refused),
                Err(DynamicsError::InvalidBody {
                    body: DynamicsBodyId(2)
                })
            );
        }
        assert_eq!(output(&solver, 1).rotation, moved);
    }

    #[test]
    fn locked_axes_ignore_initial_velocity_and_influences() {
        let mut body = body();
        body.locked_translation_axes = [true; 3];
        body.locked_rotation_axes = [true; 3];
        body.linear_velocity = [1.0, 2.0, 3.0];
        body.angular_velocity = [1.0, 0.0, 0.0];
        let mut solver = solver(GRAVITY, &[body]);
        let mut action = DynamicsAction::impulse(body.id, [5.0; 3]);
        action.force = [5.0; 3];
        action.torque_impulse = [5.0; 3];
        solver.step(TICK, 1, &[action]).unwrap();
        let after = output(&solver, 1);
        assert_eq!(after.translation, [0.0; 3]);
        assert_eq!(after.linear_velocity, [0.0; 3]);
        assert_eq!(after.angular_velocity, [0.0; 3]);
    }

    #[test]
    fn an_action_force_lasts_for_its_own_call() {
        let mut solver = solver([0.0; 3], &[body()]);
        let mut push = DynamicsAction::impulse(DynamicsBodyId(1), [0.0; 3]);
        push.force = [6.0, 0.0, 0.0];
        solver.step(TICK, 2, &[push]).unwrap();
        let pushed = output(&solver, 1).linear_velocity[0];
        assert!((pushed - 6.0 * 2.0 * TICK).abs() < 1e-9);
        solver.step(TICK, 2, &[]).unwrap();
        assert!((output(&solver, 1).linear_velocity[0] - pushed).abs() < 1e-12);
    }

    /// A capsule dropped on its side onto the voxel floor comes to rest and
    /// sleeps (#10203). Lying exactly balanced, it either settles or, under
    /// parry 0.29's contacts, rolled on forever; most of these drops did.
    #[test]
    fn a_capsule_dropped_on_its_side_sleeps() {
        const DROPS: [(f64, f64); 8] = [
            (0.13, 0.0),
            (0.16, 0.002),
            (0.19, 0.004),
            (0.22, 0.0),
            (0.25, 0.006),
            (0.28, 0.002),
            (0.31, 0.008),
            (0.34, 0.004),
        ];
        for (height, tilt) in DROPS {
            let world = floor_world(&[0]);
            let mut solver = DynamicsSolver::new(GRAVITY);
            solver.bind_environment(&CollisionProjection::build(&world));
            let mut capsule = body();
            capsule.shape = DynamicsShape::CapsuleY {
                half_height: 0.3,
                radius: 0.12,
            };
            // Turned a quarter about z onto its side, after a small tip
            // about x.
            let side = std::f64::consts::FRAC_PI_4.sin();
            let (sin, cos) = (tilt * 0.5).sin_cos();
            capsule.rotation = [side * sin, side * sin, side * cos, side * cos];
            capsule.translation = [4.0, height, 4.0];
            capsule.gravity_scale = 1.0;
            solver.insert_body(capsule).unwrap();
            settle(&mut solver, 240);
            let rested = output(&solver, 1);
            assert!(
                rested.sleeping,
                "dropped from {height} m: still moving at {:?} rad/s",
                rested.angular_velocity
            );
        }
    }

    #[test]
    fn a_retained_stack_settles_sleeps_and_keeps_its_contacts() {
        let world = floor_world(&[0]);
        let mut solver = DynamicsSolver::new(GRAVITY);
        let receipt = solver.bind_environment(&CollisionProjection::build(&world));
        assert_eq!(receipt.inserted, 1);
        for level in 0..4 {
            let mut crate_body = body();
            crate_body.id = DynamicsBodyId(level + 1);
            crate_body.shape = DynamicsShape::Cuboid {
                half_extents: [0.4; 3],
            };
            crate_body.translation = [4.0, 0.41 + 0.82 * level as f64, 4.0];
            crate_body.gravity_scale = 1.0;
            solver.insert_body(crate_body).unwrap();
        }
        settle(&mut solver, 240);
        for level in 0..4 {
            let settled = output(&solver, level + 1);
            assert!(settled.sleeping, "crate {level} is still awake");
            assert!(
                (settled.translation[1] - (0.4 + 0.8 * level as f64)).abs() < 0.05,
                "crate {level} at {:?}",
                settled.translation
            );
        }
        // One floor contact and three stacked contacts.
        assert_eq!(solver.contacts().len(), 4);
        assert_eq!(
            solver
                .contacts()
                .iter()
                .filter(|contact| contact.second.is_none())
                .count(),
            1
        );
    }

    #[test]
    fn environment_rebind_changes_only_edited_chunks_and_wakes_what_they_held() {
        let mut world = floor_world(&[0, 1]);
        let mut projection = CollisionProjection::build(&world);
        let mut solver = DynamicsSolver::new(GRAVITY);
        assert_eq!(solver.bind_environment(&projection).inserted, 2);
        for (id, x) in [(1, 4.0), (2, 12.0)] {
            let mut ball = body();
            ball.id = DynamicsBodyId(id);
            ball.translation = [x, 0.5, 4.0];
            ball.gravity_scale = 1.0;
            solver.insert_body(ball).unwrap();
        }
        settle(&mut solver, 180);
        assert!(output(&solver, 1).sleeping && output(&solver, 2).sleeping);

        // Rebinding an unchanged projection leaves the world alone.
        let unchanged = solver.bind_environment(&projection.clone());
        assert_eq!(
            (unchanged.retained, unchanged.inserted, unchanged.removed),
            (2, 0, 0)
        );

        // Dig out the floor under the second ball.
        let coord = ChunkCoord::new(1, -1, 0);
        world.insert(coord, VoxelChunk::from_spec(&spec()));
        projection.rebuild_chunk(&world, coord);
        let edited = solver.bind_environment(&projection);
        assert_eq!(
            (edited.retained, edited.inserted, edited.removed),
            (1, 0, 1)
        );
        settle(&mut solver, 30);
        assert!(output(&solver, 1).sleeping);
        assert!(output(&solver, 2).translation[1] < -0.5);
    }

    #[test]
    fn removing_a_body_drops_its_ropes_and_replacing_one_keeps_them() {
        let mut first = body();
        first.translation = [0.0, -2.0, 0.0];
        let mut second = first;
        second.id = DynamicsBodyId(2);
        second.translation = [0.0, -4.0, 0.0];
        let mut solver = solver([0.0; 3], &[first, second]);
        solver.set_tether(tether()).unwrap();
        let mut link = tether();
        link.id = 8;
        link.first = DynamicsTetherEndpoint::Body {
            body: first.id,
            local_anchor: [0.0; 3],
        };
        link.second = DynamicsTetherEndpoint::Body {
            body: second.id,
            local_anchor: [0.0; 3],
        };
        solver.set_tether(link).unwrap();

        let mut heavier = first;
        heavier.mass = 5.0;
        heavier.shape = DynamicsShape::Cuboid {
            half_extents: [0.3; 3],
        };
        solver.replace_body(heavier).unwrap();
        assert_eq!(solver.tether_count(), 2);
        solver.step(TICK, 1, &[]).unwrap();
        assert_eq!(solver.tether_readouts().len(), 2);

        assert_eq!(solver.remove_body(first.id), vec![7, 8]);
        assert_eq!(solver.tether_count(), 0);
        assert!(solver.tether_readouts().is_empty());
        solver.step(TICK, 1, &[]).unwrap();
        assert!(solver.body(first.id).is_none());
        assert!(solver.body(second.id).is_some());
    }

    #[test]
    fn translate_moves_bodies_and_fixed_rope_anchors_together() {
        let mut body = body();
        body.translation = [0.0, -3.0, 0.0];
        body.gravity_scale = 1.0;
        let mut solver = solver(GRAVITY, &[body]);
        solver.set_tether(tether()).unwrap();
        settle(&mut solver, 10);
        let before = output(&solver, 1);
        solver.translate([-16.0, 0.0, 8.0]);
        let after = output(&solver, 1);
        assert_eq!(after.translation[0], before.translation[0] - 16.0);
        assert_eq!(after.translation[2], before.translation[2] + 8.0);
        assert_eq!(after.linear_velocity, before.linear_velocity);
        assert_eq!(
            solver.tether(7).unwrap().first,
            DynamicsTetherEndpoint::Fixed([-16.0, 0.0, 8.0])
        );
        solver.step(TICK, 1, &[]).unwrap();
        let readout = solver.tether_readouts()[0];
        assert!(readout.taut && !readout.caught);
        assert!((readout.distance - 3.0).abs() < 0.001);
    }

    #[test]
    fn a_sleeping_body_stays_asleep_until_woken() {
        let mut sleeper = body();
        sleeper.sleeping = true;
        sleeper.gravity_scale = 1.0;
        let mut solver = solver(GRAVITY, &[sleeper]);
        settle(&mut solver, 5);
        assert!(output(&solver, 1).sleeping);
        assert_eq!(output(&solver, 1).translation, [0.0; 3]);
        solver
            .step(
                TICK,
                1,
                &[DynamicsAction::impulse(sleeper.id, [0.0, 1.0, 0.0])],
            )
            .unwrap();
        assert!(!output(&solver, 1).sleeping);
    }

    /// A small, dense capsule rolling on its side (a severed arm, #10203)
    /// comes to rest: without rolling resistance it crept on at a few cm/s
    /// under angular damping 3, and even 10.
    #[test]
    fn a_capsule_rolling_on_its_side_comes_to_rest() {
        // On voxel ground (a compound of boxes) and on a two-triangle mesh,
        // whose contacts the solver clusters.
        for voxel in [true, false] {
            let mut solver = DynamicsSolver::new(GRAVITY);
            if voxel {
                solver.bind_environment(&CollisionProjection::build(&floor_world(&[-1, 0])));
            } else {
                let corners = [[-10.0, -10.0], [10.0, -10.0], [10.0, 10.0], [-10.0, 10.0]]
                    .map(|[x, z]| Vector::new(x, 0.0, z));
                let floor =
                    ColliderBuilder::trimesh(corners.to_vec(), vec![[0, 2, 1], [0, 3, 2]]).unwrap();
                solver.world.insert_collider(floor, None);
            }
            rolls_to_rest(solver);
        }
    }

    fn rolls_to_rest(mut solver: DynamicsSolver) {
        let mut arm = body();
        arm.shape = DynamicsShape::CapsuleY {
            half_height: 0.22,
            radius: 0.07,
        };
        arm.mass = 4.0;
        arm.friction = 0.8;
        arm.restitution = 0.1;
        arm.linear_damping = 0.2;
        arm.angular_damping = 3.0;
        arm.gravity_scale = 1.0;
        arm.translation = [0.5, 1.5, 0.4];
        let side = std::f64::consts::FRAC_PI_4.sin();
        arm.rotation = [0.0, 0.0, side, side];
        arm.linear_velocity = [1.2, 1.0, -0.3];
        arm.angular_velocity = [0.0, 3.0, 2.0];
        solver.insert_body(arm).unwrap();
        settle(&mut solver, 240);
        let rested = output(&solver, 1);
        assert!(
            rested.sleeping,
            "still rolling at {:?} m/s",
            rested.linear_velocity
        );
    }

    /// A ball rolling across the floor slows steadily under rolling
    /// resistance and comes to rest, rather than rolling for ever.
    /// Rolling resistance keeps a body's locked rotation axes still
    /// (R10203-1): on a contact sloped across a locked axis, slowing only the
    /// rolling part of the spin would leave a component along that axis.
    #[test]
    fn rolling_resistance_keeps_locked_rotation_axes_still() {
        let normal = Vector::new(1.0, 1.0, 0.0).normalize();
        let loss = ROLLING_RESISTANCE * 9.81 * TICK / 0.5;
        let spin = Vector::new(0.0, 1.0, 0.0);
        let unlocked = resisted_spin(spin, normal, loss, [false; 3]);
        assert!(
            unlocked.x > 1e-3,
            "slowing alone turns about x: {unlocked:?}"
        );
        let resisted = resisted_spin(spin, normal, loss, [true, false, false]);
        assert_eq!(resisted.x, 0.0, "{resisted:?}");
        assert_eq!(resisted.z, 0.0, "{resisted:?}");
        assert!(
            resisted.y > 0.99 && resisted.y < 1.0,
            "the rolling slows: {resisted:?}"
        );

        // A sphere locked about x, spinning about y on a 45° slope.
        let mut solver = DynamicsSolver::new(GRAVITY);
        let slope = ColliderBuilder::cuboid(4.0, 0.5, 4.0).rotation(Vector::new(
            0.0,
            0.0,
            std::f64::consts::FRAC_PI_4,
        ));
        solver.world.insert_collider(slope, None);
        let mut ball = body();
        ball.shape = DynamicsShape::Sphere { radius: 0.5 };
        ball.gravity_scale = 1.0;
        ball.locked_rotation_axes = [true, false, false];
        let up = Vector::new(-1.0, 1.0, 0.0).normalize();
        ball.translation = (up * 1.0).to_array();
        ball.angular_velocity = [0.0, 1.0, 0.0];
        solver.insert_body(ball).unwrap();
        let mut touched = false;
        for _ in 0..60 {
            solver.step(TICK, 1, &[]).unwrap();
            touched |= !solver.contacts().is_empty();
            let spin = output(&solver, 1).angular_velocity;
            assert_eq!(spin[0], 0.0, "the locked x axis turned: {spin:?}");
        }
        assert!(touched, "the ball rests on the slope");
    }

    #[test]
    fn rolling_resistance_brings_a_rolling_ball_to_rest() {
        let world = floor_world(&[0, 1, 2, 3, 4, 5]);
        let mut solver = DynamicsSolver::new(GRAVITY);
        solver.bind_environment(&CollisionProjection::build(&world));
        let mut ball = body();
        ball.shape = DynamicsShape::Sphere { radius: 0.11 };
        ball.mass = 0.45;
        ball.gravity_scale = 1.0;
        ball.translation = [1.0, 0.11, 4.0];
        ball.linear_velocity = [2.0, 0.0, 0.0];
        ball.angular_velocity = [0.0, 0.0, -2.0 / 0.11];
        solver.insert_body(ball).unwrap();
        settle(&mut solver, 240);
        let rolling = output(&solver, 1).linear_velocity[0];
        assert!(
            rolling > 1.5 && rolling < 1.9,
            "after 4 s it rolls at {rolling} m/s"
        );
        settle(&mut solver, 60 * 40);
        let rested = output(&solver, 1);
        assert!(rested.sleeping, "{rested:?}");
        assert!(
            rested.translation[0] > 20.0 && rested.translation[0] < 45.0,
            "it rolled to {:?}",
            rested.translation
        );
    }
}
