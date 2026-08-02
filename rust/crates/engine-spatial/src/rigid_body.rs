use std::collections::BTreeMap;

use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{
    replace_rigid_body_states, ComponentRevision, EntityLifecycle, EntityState, EntityTransform,
    KinematicComponent, Quat, RigidBodyComponent, RigidBodyShape, RigidBodyStatePublicationError,
    RigidBodyStateReplacement, TransformComponent,
};
use svc_collision::{
    simulate_dynamics, DynamicsAction, DynamicsBodyId, DynamicsBodyInput, DynamicsContact,
    DynamicsError, DynamicsShape, DynamicsStepInput, DynamicsStepOutput,
};

use crate::VoxelCollisionScene;

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

#[derive(Debug, Default)]
pub struct RigidBodyService {
    generation: u64,
    last_readout: Option<RigidBodyWorldReadout>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RigidBodyEnvironmentIdentity {
    source_revision: u64,
    authority_hash: u64,
    collision_projection_version: u64,
    static_mesh_revision: u64,
}

impl RigidBodyService {
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
        let candidate = simulate_dynamics(&scene.projection, input)?;
        Ok(PreparedRigidBodyStep {
            canonical,
            candidate,
            steps: request.steps,
            environment: environment_identity(scene),
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
        } = prepared;
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
        let publication = replace_rigid_body_states(entities, replacements)?;
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
    DynamicsBodyInput {
        id: DynamicsBodyId(body.entity.raw()),
        translation: vec3_f64(body.transform.translation),
        rotation: [
            f64::from(body.transform.rotation.x),
            f64::from(body.transform.rotation.y),
            f64::from(body.transform.rotation.z),
            f64::from(body.transform.rotation.w),
        ],
        shape: match body.body.shape {
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
        mass: f64::from(body.body.mass),
        linear_velocity: vec3_f64(body.body.linear_velocity),
        angular_velocity: vec3_f64(body.body.angular_velocity),
        linear_damping: f64::from(body.body.linear_damping),
        angular_damping: f64::from(body.body.angular_damping),
        gravity_scale: f64::from(body.body.gravity_scale),
        friction: f64::from(body.body.friction),
        restitution: f64::from(body.body.restitution),
        collision_groups: body.body.collision_groups,
        collision_mask: body.body.collision_mask,
        enabled: body.body.enabled,
        sleeping: body.body.sleeping,
        continuous_collision: body.body.continuous_collision,
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
