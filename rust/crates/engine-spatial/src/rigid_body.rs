use std::collections::BTreeMap;

use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{
    replace_rigid_body_states, ComponentRevision, EntityLifecycle, EntityState, EntityTransform,
    KinematicComponent, Quat, RigidBodyComponent, RigidBodyShape, RigidBodyStatePublicationError,
    RigidBodyStateReplacement, TransformComponent,
};
use svc_collision::{
    simulate_dynamics_with_rope_solver, validate_dynamics_tethers, DynamicsAction, DynamicsBodyId,
    DynamicsBodyInput, DynamicsContact, DynamicsError, DynamicsMassProperties,
    DynamicsRopeSolverConfig, DynamicsShape, DynamicsStepInput, DynamicsStepOutput, DynamicsTether,
    DynamicsTetherEndpoint, DynamicsTetherReadout,
};

use crate::VoxelCollisionScene;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBodyAnchorObservation {
    pub point: Vec3,
    pub point_velocity: Vec3,
    pub center_of_mass: Vec3,
    pub response: [Vec3; 3],
}

pub fn observe_rigid_body_anchor(
    entities: &EntityState,
    entity: EntityId,
    local_anchor: Vec3,
) -> Result<RigidBodyAnchorObservation, RigidBodyStepError> {
    if entities.lifecycle(entity) != Some(EntityLifecycle::Active) {
        return Err(RigidBodyStepError::InactiveBody { entity });
    }
    if entities.transform_parent(entity).is_some() {
        return Err(RigidBodyStepError::ParentedBody { entity });
    }
    let transform = entities
        .transform(entity)
        .copied()
        .ok_or(RigidBodyStepError::MissingTransform { entity })?;
    if transform.scale != Vec3::ONE {
        return Err(RigidBodyStepError::NonUnitScale { entity });
    }
    let body = entities
        .rigid_body(entity)
        .copied()
        .ok_or(RigidBodyStepError::InactiveBody { entity })?;
    let observation = svc_collision::observe_dynamics_anchor(
        component_body_input(entity, transform, body),
        vec3_f64(local_anchor),
    )?;
    let convert = |value| vec3_f32(value).ok_or(RigidBodyStepError::OutputOutOfRange { entity });
    Ok(RigidBodyAnchorObservation {
        point: convert(observation.point)?,
        point_velocity: convert(observation.point_velocity)?,
        center_of_mass: convert(observation.center_of_mass)?,
        response: [
            convert(observation.response[0])?,
            convert(observation.response[1])?,
            convert(observation.response[2])?,
        ],
    })
}

/// Purpose-neutral mass facts for an admitted dynamic primitive. The native C#
/// bridge reports these values so product control code can use Engine's shape
/// and mass policy without copying an inertia formula downstream.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBodyMassProperties {
    pub mass: f32,
    pub center_of_mass: Vec3,
    pub principal_inertia: Vec3,
    pub principal_inertia_local_frame: Quat,
}

pub fn rigid_body_mass_properties(
    shape: RigidBodyShape,
    mass: f32,
) -> Option<RigidBodyMassProperties> {
    let body = RigidBodyComponent::dynamic(shape, mass);
    entity_state::validate_rigid_body(&body).ok()?;
    let principal_inertia = match shape {
        // Ixx = m / 12 * ((2hy)^2 + (2hz)^2), and cyclic permutations.
        RigidBodyShape::Cuboid { half_extents } => {
            let scale = mass / 3.0;
            Vec3::new(
                scale * (half_extents.y * half_extents.y + half_extents.z * half_extents.z),
                scale * (half_extents.x * half_extents.x + half_extents.z * half_extents.z),
                scale * (half_extents.x * half_extents.x + half_extents.y * half_extents.y),
            )
        }
        RigidBodyShape::Sphere { radius } => {
            let inertia = 0.4 * mass * radius * radius;
            Vec3::new(inertia, inertia, inertia)
        }
        // Capsule admission is intentionally not part of the generated C#
        // Dynamics family yet, so it has no readout policy in this slice.
        RigidBodyShape::CapsuleY { .. } => return None,
    };
    Some(RigidBodyMassProperties {
        mass,
        center_of_mass: Vec3::ZERO,
        principal_inertia,
        principal_inertia_local_frame: Quat::IDENTITY,
    })
}

/// Return the exact mass-property tuple selected by an authored body.
///
/// The shape/mass helper above remains the compatibility entry point for
/// callers that want Engine's derived defaults. This body-oriented readout
/// preserves an explicit policy without asking a downstream caller to repeat
/// the policy selection.
pub fn rigid_body_component_mass_properties(
    body: RigidBodyComponent,
) -> Option<RigidBodyMassProperties> {
    entity_state::validate_rigid_body(&body).ok()?;
    match body.inertia {
        entity_state::RigidBodyInertiaPolicy::DeriveFromShapeAndMass => {
            rigid_body_mass_properties(body.shape, body.mass)
        }
        entity_state::RigidBodyInertiaPolicy::Explicit {
            center_of_mass,
            principal_inertia,
            principal_inertia_local_frame,
        } => Some(RigidBodyMassProperties {
            mass: body.mass,
            center_of_mass,
            principal_inertia,
            principal_inertia_local_frame,
        }),
    }
}

/// Compatibility helper for existing cuboid callers.
pub fn cuboid_mass_properties(half_extents: Vec3, mass: f32) -> Option<RigidBodyMassProperties> {
    rigid_body_mass_properties(RigidBodyShape::Cuboid { half_extents }, mass)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBodyAction {
    pub entity: EntityId,
    pub force: Vec3,
    pub torque: Vec3,
    pub impulse: Vec3,
    pub torque_impulse: Vec3,
    pub wake: bool,
}

impl RigidBodyAction {
    pub const fn impulse(entity: EntityId, impulse: Vec3) -> Self {
        Self {
            entity,
            force: Vec3::ZERO,
            torque: Vec3::ZERO,
            impulse,
            torque_impulse: Vec3::ZERO,
            wake: true,
        }
    }

    pub const fn force(entity: EntityId, force: Vec3) -> Self {
        Self {
            entity,
            force,
            torque: Vec3::ZERO,
            impulse: Vec3::ZERO,
            torque_impulse: Vec3::ZERO,
            wake: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RigidBodyStepRequest {
    pub step_seconds: f32,
    pub steps: u8,
    pub gravity: Vec3,
    pub actions: Vec<RigidBodyAction>,
}

impl RigidBodyStepRequest {
    pub fn single(step_seconds: f32) -> Self {
        Self {
            step_seconds,
            steps: 1,
            gravity: Vec3::new(0.0, -9.81, 0.0),
            actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBodyContactReadout {
    pub first: EntityId,
    pub second: Option<EntityId>,
    pub impulse: Vec3,
    pub impulse_magnitude: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBodyMotionFact {
    pub entity: EntityId,
    pub transform_before: EntityTransform,
    pub transform_after: EntityTransform,
    pub linear_velocity_before: Vec3,
    pub linear_velocity_after: Vec3,
    pub angular_velocity_before: Vec3,
    pub angular_velocity_after: Vec3,
    pub sleeping_before: bool,
    pub sleeping_after: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RigidBodyStepReceipt {
    pub generation: u64,
    pub revision_before: u64,
    pub revision_after: u64,
    pub steps: u8,
    pub bodies_considered: usize,
    pub moved_bodies: usize,
    pub slept_bodies: usize,
    pub woken_bodies: usize,
    pub facts: Vec<RigidBodyMotionFact>,
    pub contacts: Vec<RigidBodyContactReadout>,
    pub tethers: Vec<DynamicsTetherReadout>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RigidBodyWorldReadout {
    pub generation: u64,
    pub body_count: usize,
    pub contact_count: usize,
    pub entity_revision: u64,
}

#[derive(Debug)]
pub enum RigidBodyStepError {
    StaleTethers,
    TetherOutputOutOfRange { id: u64 },
    TetherOutOfReach { id: u64 },
    MissingTransform { entity: EntityId },
    KinematicConflict { entity: EntityId },
    ParentedBody { entity: EntityId },
    NonUnitScale { entity: EntityId },
    InactiveBody { entity: EntityId },
    Backend(DynamicsError),
    Publication(RigidBodyStatePublicationError),
    OutputOutOfRange { entity: EntityId },
    StaleBodySet,
    StaleRelationship { entity: EntityId },
    StaleEnvironment,
    GenerationExhausted,
}

impl RigidBodyStepError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::TetherOutputOutOfRange { .. } => "rigid-body-tether-output-out-of-range",
            Self::StaleTethers => "stale-rigid-body-tethers",
            Self::TetherOutOfReach { .. } => "rigid-body-tether-out-of-reach",
            Self::MissingTransform { .. } => "missing-rigid-body-transform",
            Self::KinematicConflict { .. } => "kinematic-rigid-body-conflict",
            Self::ParentedBody { .. } => "parented-rigid-body-transform",
            Self::NonUnitScale { .. } => "scaled-rigid-body-transform",
            Self::InactiveBody { .. } => "inactive-rigid-body",
            Self::Backend(error) => error.code(),
            Self::Publication(error) => error.code(),
            Self::OutputOutOfRange { .. } => "rigid-body-output-out-of-range",
            Self::StaleBodySet => "stale-rigid-body-set",
            Self::StaleRelationship { .. } => "stale-rigid-body-relationship",
            Self::StaleEnvironment => "stale-rigid-body-environment",
            Self::GenerationExhausted => "rigid-body-generation-exhausted",
        }
    }
}

impl std::fmt::Display for RigidBodyStepError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.code())
    }
}

impl std::error::Error for RigidBodyStepError {}

impl From<DynamicsError> for RigidBodyStepError {
    fn from(value: DynamicsError) -> Self {
        Self::Backend(value)
    }
}

impl From<RigidBodyStatePublicationError> for RigidBodyStepError {
    fn from(value: RigidBodyStatePublicationError) -> Self {
        Self::Publication(value)
    }
}

/// Canonical rope continuation, separate from derived Rapier state.
#[derive(Debug, Clone, PartialEq)]
pub struct RigidBodyRopeSnapshot {
    pub definitions: Vec<DynamicsTether>,
    pub solver: DynamicsRopeSolverConfig,
}

#[derive(Debug, Default, Clone)]
pub struct RigidBodyService {
    generation: u64,
    last_readout: Option<RigidBodyWorldReadout>,
    tethers: Vec<DynamicsTether>,
    tether_revision: u64,
    rope_solver: DynamicsRopeSolverConfig,
}

#[derive(Clone)]
struct CanonicalBody {
    entity: EntityId,
    transform_revision: ComponentRevision,
    body_revision: ComponentRevision,
    transform: TransformComponent,
    body: RigidBodyComponent,
}

/// Opaque off-side dynamics candidate bound to exact entity component revisions.
///
/// Callers may perform other work between preparation and commit. Publication
/// then either replaces every admitted transform/body pair atomically or
/// rejects the complete candidate when any captured slot has changed.
#[derive(Clone)]
pub struct PreparedRigidBodyStep {
    canonical: Vec<CanonicalBody>,
    candidate: DynamicsStepOutput,
    steps: u8,
    environment: RigidBodyEnvironmentIdentity,
    tether_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RigidBodyEnvironmentIdentity {
    source_revision: u64,
    authority_hash: u64,
    collision_projection_version: u64,
    static_mesh_revision: u64,
}

impl RigidBodyService {
    pub fn capture_ropes(&self) -> RigidBodyRopeSnapshot {
        RigidBodyRopeSnapshot {
            definitions: self.tethers.clone(),
            solver: self.rope_solver,
        }
    }

    /// Restore alongside the matching body snapshot. A solver's accepted small
    /// extension is continuation state, not a new out-of-reach attachment.
    pub fn restore_ropes(
        &mut self,
        entities: &EntityState,
        mut snapshot: RigidBodyRopeSnapshot,
    ) -> Result<(), RigidBodyStepError> {
        snapshot.solver.validate().map_err(DynamicsError::Tether)?;
        let canonical = collect_canonical_bodies(entities)?;
        let bodies: Vec<_> = canonical.iter().map(body_input).collect();
        validate_dynamics_tethers(&mut snapshot.definitions, &bodies)
            .map_err(DynamicsError::Tether)?;
        let revision = self
            .tether_revision
            .checked_add(1)
            .ok_or(RigidBodyStepError::GenerationExhausted)?;
        self.tethers = snapshot.definitions;
        self.rope_solver = snapshot.solver;
        self.tether_revision = revision;
        Ok(())
    }

    pub fn rope_solver(&self) -> DynamicsRopeSolverConfig {
        self.rope_solver
    }

    pub fn configure_rope_solver(
        &mut self,
        config: DynamicsRopeSolverConfig,
    ) -> Result<(), RigidBodyStepError> {
        config.validate().map_err(DynamicsError::Tether)?;
        let revision = self
            .tether_revision
            .checked_add(1)
            .ok_or(RigidBodyStepError::GenerationExhausted)?;
        self.rope_solver = config;
        self.tether_revision = revision;
        Ok(())
    }

    pub fn tether_count(&self) -> usize {
        self.tethers.len()
    }

    pub fn tether(&self, id: u64) -> Option<DynamicsTether> {
        self.tethers.iter().find(|tether| tether.id == id).copied()
    }

    /// Rebase fixed points with the same coordinate primitive as rigid bodies.
    /// Body-local anchors, lengths and transition facts are unchanged.
    pub fn rebase_tethers(
        &mut self,
        before: core_space::WorldOrigin,
        after: core_space::WorldOrigin,
        envelope: f32,
    ) -> Result<(), RigidBodyStepError> {
        let mut candidate = self.tethers.clone();
        for tether in &mut candidate {
            for endpoint in [&mut tether.first, &mut tether.second] {
                if let DynamicsTetherEndpoint::Fixed(point) = endpoint {
                    let invalid = || RigidBodyStepError::TetherOutOfReach { id: tether.id };
                    let local = core_space::GlobalPosition::from_local(
                        before,
                        [point[0] as f32, point[1] as f32, point[2] as f32],
                    )
                    .map_err(|_| invalid())?
                    .local(after, envelope)
                    .map_err(|_| invalid())?;
                    *point = local.map(f64::from);
                }
            }
        }
        let revision = self
            .tether_revision
            .checked_add(1)
            .ok_or(RigidBodyStepError::GenerationExhausted)?;
        self.tethers = candidate;
        self.tether_revision = revision;
        Ok(())
    }

    /// Copied canonical continuation: definitions, effective lengths and transition facts.
    pub fn capture_tethers(&self) -> Vec<DynamicsTether> {
        self.tethers.clone()
    }

    /// Admit a complete authored set. Existing effective lengths must be retained
    /// during reeling; target_length is the live control. Empty removes all tethers.
    pub fn replace_tethers(
        &mut self,
        entities: &EntityState,
        mut tethers: Vec<DynamicsTether>,
    ) -> Result<(), RigidBodyStepError> {
        let canonical = collect_canonical_bodies(entities)?;
        let bodies: Vec<_> = canonical.iter().map(body_input).collect();
        validate_dynamics_tethers(&mut tethers, &bodies).map_err(DynamicsError::Tether)?;
        for tether in &mut tethers {
            let previous = self.tethers.iter().find(|old| old.id == tether.id);
            let same_attachment = previous
                .is_some_and(|old| old.first == tether.first && old.second == tether.second);
            // A direct edit cannot bypass rate-limited length control. Restore
            // uses restore_ropes instead of this authoring path.
            if let Some(old) = previous.filter(|_| same_attachment) {
                tether.maximum_length = old.maximum_length;
                tether.was_taut = old.was_taut;
            } else {
                let point = |endpoint| -> Vec3 {
                    match endpoint {
                        DynamicsTetherEndpoint::Fixed(point) => {
                            Vec3::new(point[0] as f32, point[1] as f32, point[2] as f32)
                        }
                        DynamicsTetherEndpoint::Body { body, local_anchor } => {
                            let body = canonical
                                .iter()
                                .find(|candidate| candidate.entity.raw() == body.0)
                                .expect("validated anchor");
                            body.transform.transform().transform_point(Vec3::new(
                                local_anchor[0] as f32,
                                local_anchor[1] as f32,
                                local_anchor[2] as f32,
                            ))
                        }
                    }
                };
                let distance = (point(tether.second) - point(tether.first)).length();
                if !distance.is_finite() || f64::from(distance) > tether.maximum_length + 0.001 {
                    return Err(RigidBodyStepError::TetherOutOfReach { id: tether.id });
                }
            }
            tether.wake |= previous.is_some_and(|old| old.wake || old != tether);
        }
        let revision = self
            .tether_revision
            .checked_add(1)
            .ok_or(RigidBodyStepError::GenerationExhausted)?;
        self.tethers = tethers;
        self.tether_revision = revision;
        Ok(())
    }

    pub fn readout(&self) -> Option<&RigidBodyWorldReadout> {
        self.last_readout.as_ref()
    }

    pub fn step(
        &mut self,
        entities: &mut EntityState,
        scene: &VoxelCollisionScene,
        request: RigidBodyStepRequest,
    ) -> Result<RigidBodyStepReceipt, RigidBodyStepError> {
        let prepared = self.prepare(entities, scene, request)?;
        self.commit(entities, scene, prepared)
    }

    pub fn prepare(
        &self,
        entities: &EntityState,
        scene: &VoxelCollisionScene,
        request: RigidBodyStepRequest,
    ) -> Result<PreparedRigidBodyStep, RigidBodyStepError> {
        let canonical = collect_canonical_bodies(entities)?;
        let input = DynamicsStepInput {
            step_seconds: f64::from(request.step_seconds),
            steps: request.steps,
            gravity: vec3_f64(request.gravity),
            bodies: canonical.iter().map(body_input).collect(),
            actions: request.actions.iter().map(action_input).collect(),
        };
        let candidate = simulate_dynamics_with_rope_solver(
            &scene.projection,
            input,
            self.tethers.clone(),
            self.rope_solver,
        )?;
        Ok(PreparedRigidBodyStep {
            canonical,
            candidate,
            steps: request.steps,
            environment: environment_identity(scene),
            tether_revision: self.tether_revision,
        })
    }

    pub fn commit(
        &mut self,
        entities: &mut EntityState,
        scene: &VoxelCollisionScene,
        prepared: PreparedRigidBodyStep,
    ) -> Result<RigidBodyStepReceipt, RigidBodyStepError> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(RigidBodyStepError::GenerationExhausted)?;
        let PreparedRigidBodyStep {
            canonical,
            candidate,
            steps,
            environment,
            tether_revision,
        } = prepared;
        if tether_revision != self.tether_revision {
            return Err(RigidBodyStepError::StaleTethers);
        }
        let next_tether_revision = self
            .tether_revision
            .checked_add(1)
            .ok_or(RigidBodyStepError::GenerationExhausted)?;
        if environment != environment_identity(scene) {
            return Err(RigidBodyStepError::StaleEnvironment);
        }
        validate_prepared_entity_authority(entities, &canonical)?;
        let output: BTreeMap<_, _> = candidate
            .bodies
            .into_iter()
            .map(|body| (body.id, body))
            .collect();
        let mut replacements = Vec::with_capacity(canonical.len());
        let mut facts = Vec::with_capacity(canonical.len());
        for before in &canonical {
            let after = output
                .get(&DynamicsBodyId(before.entity.raw()))
                .expect("backend returns every admitted body");
            let transform = TransformComponent {
                translation: vec3_f32(after.translation).ok_or(
                    RigidBodyStepError::OutputOutOfRange {
                        entity: before.entity,
                    },
                )?,
                rotation: quat_f32(after.rotation).ok_or(RigidBodyStepError::OutputOutOfRange {
                    entity: before.entity,
                })?,
                scale: Vec3::ONE,
            };
            let mut body = before.body;
            body.linear_velocity =
                vec3_f32(after.linear_velocity).ok_or(RigidBodyStepError::OutputOutOfRange {
                    entity: before.entity,
                })?;
            body.angular_velocity =
                vec3_f32(after.angular_velocity).ok_or(RigidBodyStepError::OutputOutOfRange {
                    entity: before.entity,
                })?;
            body.sleeping = after.sleeping;
            facts.push(RigidBodyMotionFact {
                entity: before.entity,
                transform_before: before.transform.transform(),
                transform_after: transform.transform(),
                linear_velocity_before: before.body.linear_velocity,
                linear_velocity_after: body.linear_velocity,
                angular_velocity_before: before.body.angular_velocity,
                angular_velocity_after: body.angular_velocity,
                sleeping_before: before.body.sleeping,
                sleeping_after: body.sleeping,
            });
            replacements.push(RigidBodyStateReplacement {
                entity: before.entity,
                expected_transform_revision: before.transform_revision.clone(),
                expected_rigid_body_revision: before.body_revision.clone(),
                transform,
                rigid_body: body,
            });
        }
        let contacts = candidate
            .contacts
            .into_iter()
            .map(contact_readout)
            .collect::<Result<Vec<_>, _>>()?;
        for tether in &candidate.tethers {
            if vec3_f32(tether.first).is_none()
                || vec3_f32(tether.second).is_none()
                || ![tether.maximum_length, tether.distance, tether.force_proxy]
                    .into_iter()
                    .all(|value| (value as f32).is_finite())
            {
                return Err(RigidBodyStepError::TetherOutputOutOfRange { id: tether.id });
            }
        }
        let publication = replace_rigid_body_states(entities, replacements)?;
        for (tether, readout) in self.tethers.iter_mut().zip(&candidate.tethers) {
            tether.maximum_length = readout.maximum_length;
            tether.was_taut = readout.taut;
            tether.wake = false;
        }
        self.tether_revision = next_tether_revision;
        self.generation = generation;
        self.last_readout = Some(RigidBodyWorldReadout {
            generation,
            body_count: canonical.len(),
            contact_count: contacts.len(),
            entity_revision: publication.revision_after,
        });
        let moved_bodies = facts
            .iter()
            .filter(|fact| {
                fact.transform_before != fact.transform_after
                    || fact.linear_velocity_before != fact.linear_velocity_after
                    || fact.angular_velocity_before != fact.angular_velocity_after
            })
            .count();
        let slept_bodies = facts
            .iter()
            .filter(|fact| !fact.sleeping_before && fact.sleeping_after)
            .count();
        let woken_bodies = facts
            .iter()
            .filter(|fact| fact.sleeping_before && !fact.sleeping_after)
            .count();
        Ok(RigidBodyStepReceipt {
            generation,
            revision_before: publication.revision_before,
            revision_after: publication.revision_after,
            steps,
            bodies_considered: canonical.len(),
            moved_bodies,
            slept_bodies,
            woken_bodies,
            facts,
            contacts,
            tethers: candidate.tethers,
        })
    }
}

fn validate_prepared_entity_authority(
    entities: &EntityState,
    canonical: &[CanonicalBody],
) -> Result<(), RigidBodyStepError> {
    let current_entities = entities
        .rigid_bodies()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    if current_entities.len() != canonical.len()
        || current_entities
            .iter()
            .zip(canonical)
            .any(|(current, prepared)| *current != prepared.entity)
    {
        return Err(RigidBodyStepError::StaleBodySet);
    }
    for body in canonical {
        if entities.transform_parent(body.entity).is_some() {
            return Err(RigidBodyStepError::StaleRelationship {
                entity: body.entity,
            });
        }
    }
    Ok(())
}

fn environment_identity(scene: &VoxelCollisionScene) -> RigidBodyEnvironmentIdentity {
    RigidBodyEnvironmentIdentity {
        source_revision: scene.source_revision().raw(),
        authority_hash: scene.authority_hash(),
        collision_projection_version: scene.projection_version(),
        static_mesh_revision: scene.static_mesh_collision_revision(),
    }
}

fn collect_canonical_bodies(
    entities: &EntityState,
) -> Result<Vec<CanonicalBody>, RigidBodyStepError> {
    let mut bodies = Vec::new();
    for (entity, body) in entities.rigid_bodies() {
        let view = entities
            .view(entity)
            .expect("component cannot outlive owning entity");
        if view.lifecycle != EntityLifecycle::Active {
            return Err(RigidBodyStepError::InactiveBody { entity });
        }
        let transform = view
            .transform
            .ok_or(RigidBodyStepError::MissingTransform { entity })?;
        if view.transform_parent.is_some() {
            return Err(RigidBodyStepError::ParentedBody { entity });
        }
        if transform.scale != Vec3::ONE {
            return Err(RigidBodyStepError::NonUnitScale { entity });
        }
        if entities
            .has_component::<KinematicComponent>(entity)
            .expect("built-in kinematic registration")
        {
            return Err(RigidBodyStepError::KinematicConflict { entity });
        }
        bodies.push(CanonicalBody {
            entity,
            transform_revision: entities
                .component_revision::<TransformComponent>(entity)
                .expect("built-in transform registration"),
            body_revision: entities
                .component_revision::<RigidBodyComponent>(entity)
                .expect("built-in rigid-body registration"),
            transform,
            body: *body,
        });
    }
    Ok(bodies)
}

fn body_input(body: &CanonicalBody) -> DynamicsBodyInput {
    component_body_input(body.entity, body.transform, body.body)
}

fn component_body_input(
    entity: EntityId,
    transform: TransformComponent,
    body: RigidBodyComponent,
) -> DynamicsBodyInput {
    DynamicsBodyInput {
        id: DynamicsBodyId(entity.raw()),
        translation: vec3_f64(transform.translation),
        rotation: [
            f64::from(transform.rotation.x),
            f64::from(transform.rotation.y),
            f64::from(transform.rotation.z),
            f64::from(transform.rotation.w),
        ],
        shape: match body.shape {
            RigidBodyShape::Sphere { radius } => DynamicsShape::Sphere {
                radius: f64::from(radius),
            },
            RigidBodyShape::Cuboid { half_extents } => DynamicsShape::Cuboid {
                half_extents: vec3_f64(half_extents),
            },
            RigidBodyShape::CapsuleY {
                half_height,
                radius,
            } => DynamicsShape::CapsuleY {
                half_height: f64::from(half_height),
                radius: f64::from(radius),
            },
        },
        mass: f64::from(body.mass),
        mass_properties: match body.inertia {
            entity_state::RigidBodyInertiaPolicy::DeriveFromShapeAndMass => None,
            entity_state::RigidBodyInertiaPolicy::Explicit {
                center_of_mass,
                principal_inertia,
                principal_inertia_local_frame,
            } => Some(DynamicsMassProperties {
                center_of_mass: vec3_f64(center_of_mass),
                principal_inertia: vec3_f64(principal_inertia),
                principal_inertia_local_frame: [
                    f64::from(principal_inertia_local_frame.x),
                    f64::from(principal_inertia_local_frame.y),
                    f64::from(principal_inertia_local_frame.z),
                    f64::from(principal_inertia_local_frame.w),
                ],
            }),
        },
        linear_velocity: vec3_f64(body.linear_velocity),
        angular_velocity: vec3_f64(body.angular_velocity),
        locked_translation_axes: body.locked_translation_axes,
        locked_rotation_axes: body.locked_rotation_axes,
        linear_damping: f64::from(body.linear_damping),
        angular_damping: f64::from(body.angular_damping),
        gravity_scale: f64::from(body.gravity_scale),
        friction: f64::from(body.friction),
        restitution: f64::from(body.restitution),
        collision_groups: body.collision_groups,
        collision_mask: body.collision_mask,
        enabled: body.enabled,
        sleeping: body.sleeping,
        continuous_collision: body.continuous_collision,
    }
}

fn action_input(action: &RigidBodyAction) -> DynamicsAction {
    DynamicsAction {
        body: DynamicsBodyId(action.entity.raw()),
        force: vec3_f64(action.force),
        torque: vec3_f64(action.torque),
        impulse: vec3_f64(action.impulse),
        torque_impulse: vec3_f64(action.torque_impulse),
        wake: action.wake,
    }
}

fn contact_readout(
    contact: DynamicsContact,
) -> Result<RigidBodyContactReadout, RigidBodyStepError> {
    let first = EntityId::new(contact.first.0);
    Ok(RigidBodyContactReadout {
        first,
        second: contact.second.map(|body| EntityId::new(body.0)),
        impulse: vec3_f32(contact.impulse)
            .ok_or(RigidBodyStepError::OutputOutOfRange { entity: first })?,
        impulse_magnitude: finite_f32(contact.impulse_magnitude)
            .ok_or(RigidBodyStepError::OutputOutOfRange { entity: first })?,
    })
}

fn vec3_f64(value: Vec3) -> [f64; 3] {
    [f64::from(value.x), f64::from(value.y), f64::from(value.z)]
}

fn vec3_f32(value: [f64; 3]) -> Option<Vec3> {
    Some(Vec3::new(
        finite_f32(value[0])?,
        finite_f32(value[1])?,
        finite_f32(value[2])?,
    ))
}

fn quat_f32(value: [f64; 4]) -> Option<Quat> {
    Some(Quat::new(
        finite_f32(value[0])?,
        finite_f32(value[1])?,
        finite_f32(value[2])?,
        finite_f32(value[3])?,
    ))
}

fn finite_f32(value: f64) -> Option<f32> {
    let converted = value as f32;
    (converted.is_finite() && f64::from(converted).abs() <= f64::from(f32::MAX))
        .then_some(converted)
}
