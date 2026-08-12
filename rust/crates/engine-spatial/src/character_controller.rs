use core_ids::EntityId;
use core_math::{Vec2, Vec3};
use core_space::{WorldPos, WorldVec};
use entity_state::{
    replace_character_motion_state, CharacterMotionComponent, CharacterMotionPublicationError,
    CharacterMotionStateReplacement, CharacterStance, ComponentRevision, EntityLifecycle,
    EntityState, EntityTransform, Quat, TransformComponent,
};
use serde::{Deserialize, Serialize};
use svc_collision::{
    cast_character_capsule_against_obstacles, character_capsule_overlap_obstacles,
    CharacterCapsule, CharacterCapsuleCastHit, CharacterCapsuleOverlap,
    CharacterCollisionQueryError, CharacterCollisionSource, CharacterObstacle,
};

use crate::VoxelCollisionScene;

const PITCH_EPSILON: f32 = 0.001;

macro_rules! defaulted_config {
    ($name:ident { $($field:ident : $ty:ty = $value:expr),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
        #[serde(default, rename_all = "camelCase")]
        #[non_exhaustive]
        pub struct $name { $(pub $field: $ty),+ }

        impl Default for $name {
            fn default() -> Self { Self { $($field: $value),+ } }
        }
    };
}

defaulted_config!(CharacterShapeConfig {
    standing_height: f32 = 1.8,
    crouched_height: f32 = 1.1,
    radius: f32 = 0.35,
    contact_skin: f32 = 0.02,
    clearance_padding: f32 = 0.01,
});

defaulted_config!(CharacterGroundConfig {
    forward_speed: f32 = 5.0,
    backward_speed: f32 = 4.5,
    strafe_speed: f32 = 5.0,
    acceleration: f32 = 35.0,
    braking: f32 = 45.0,
    friction: f32 = 8.0,
    stop_speed: f32 = 2.0,
    direction_change_multiplier: f32 = 1.0,
});

defaulted_config!(CharacterAirConfig {
    maximum_speed: f32 = 5.0,
    acceleration: f32 = 12.0,
    braking: f32 = 0.0,
    wish_speed_cap: f32 = 5.0,
    lateral_control: f32 = 1.0,
    drag: f32 = 0.0,
});

defaulted_config!(CharacterVerticalConfig {
    gravity: f32 = 20.0,
    terminal_rise_speed: f32 = 55.0,
    terminal_fall_speed: f32 = 55.0,
    jump_speed: f32 = 7.0,
    grounded_downward_bias: f32 = 0.5,
});

defaulted_config!(CharacterJumpConfig {
    buffer_seconds: f32 = 0.12,
    coyote_seconds: f32 = 0.10,
    landing_lockout_seconds: f32 = 0.0,
    held_input_retriggers: bool = false,
});

defaulted_config!(CharacterSurfaceConfig {
    maximum_slope_radians: f32 = 50.0_f32.to_radians(),
    slope_hysteresis_radians: f32 = 1.0_f32.to_radians(),
    steep_slide_acceleration: f32 = 20.0,
    steep_slide_speed: f32 = 12.0,
    maximum_step_height: f32 = 0.4,
    minimum_step_width: f32 = 0.05,
    floor_snap_distance: f32 = 0.25,
    floor_snap_speed_limit: f32 = 10.0,
    ledge_support_fraction: f32 = 0.25,
});

defaulted_config!(CharacterRecoveryConfig {
    maximum_distance: f32 = 0.5,
    maximum_speed: f32 = 20.0,
    normal_nudge: f32 = 0.001,
    unresolved_tolerance: f32 = 0.002,
});

defaulted_config!(CharacterPlatformConfig {
    carry_translation: bool = true,
    carry_rotation: bool = true,
    inherit_departure_velocity: bool = true,
    departure_velocity_factor: f32 = 1.0,
    support_loss_grace_seconds: f32 = 0.0,
    crush_tolerance: f32 = 0.02,
});

defaulted_config!(CharacterExternalMotionConfig {
    impulse_scale: f32 = 1.0,
    external_decay_per_second: f32 = 0.0,
    maximum_external_speed: f32 = 50.0,
    authored_mass: f32 = 80.0,
    dynamic_impulse_factor: f32 = 1.0,
    maximum_dynamic_impulse: f32 = 500.0,
});

defaulted_config!(CharacterSolverConfig {
    maximum_slide_planes: u8 = 5,
    maximum_cast_iterations: u8 = 8,
    maximum_recovery_passes: u8 = 4,
    maximum_contacts: u16 = 32,
    maximum_step_attempts: u8 = 1,
    maximum_displacement_per_step: f32 = 10.0,
    maximum_queries_per_step: u16 = 64,
});

/// Forward-compatible character-controller policy.
///
/// External callers construct this through [`Default`], mutate public nested
/// fields, or decode a sparse document. The non-exhaustive shape prevents an
/// exhaustive external literal from turning a newly defaulted field into a
/// source break.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
#[non_exhaustive]
pub struct CharacterControllerConfig {
    pub shape: CharacterShapeConfig,
    pub ground: CharacterGroundConfig,
    pub air: CharacterAirConfig,
    pub vertical: CharacterVerticalConfig,
    pub jump: CharacterJumpConfig,
    pub surface: CharacterSurfaceConfig,
    pub recovery: CharacterRecoveryConfig,
    pub platform: CharacterPlatformConfig,
    pub external_motion: CharacterExternalMotionConfig,
    pub solver: CharacterSolverConfig,
}

impl CharacterControllerConfig {
    pub fn responsive_fps() -> Self {
        Self::default()
    }

    pub fn validate(&self) -> Result<(), CharacterConfigError> {
        positive("shape.standingHeight", self.shape.standing_height)?;
        positive("shape.crouchedHeight", self.shape.crouched_height)?;
        positive("shape.radius", self.shape.radius)?;
        if self.shape.standing_height <= self.shape.crouched_height
            || self.shape.crouched_height < self.shape.radius * 2.0
        {
            return Err(CharacterConfigError::new("shape.crouchedHeight"));
        }
        for (field, value) in [
            ("shape.contactSkin", self.shape.contact_skin),
            ("shape.clearancePadding", self.shape.clearance_padding),
            ("ground.forwardSpeed", self.ground.forward_speed),
            ("ground.backwardSpeed", self.ground.backward_speed),
            ("ground.strafeSpeed", self.ground.strafe_speed),
            ("ground.acceleration", self.ground.acceleration),
            ("ground.braking", self.ground.braking),
            ("ground.friction", self.ground.friction),
            ("ground.stopSpeed", self.ground.stop_speed),
            ("air.maximumSpeed", self.air.maximum_speed),
            ("air.acceleration", self.air.acceleration),
            ("air.braking", self.air.braking),
            ("air.wishSpeedCap", self.air.wish_speed_cap),
            ("air.drag", self.air.drag),
            ("vertical.gravity", self.vertical.gravity),
            (
                "vertical.terminalRiseSpeed",
                self.vertical.terminal_rise_speed,
            ),
            (
                "vertical.terminalFallSpeed",
                self.vertical.terminal_fall_speed,
            ),
            ("vertical.jumpSpeed", self.vertical.jump_speed),
            (
                "vertical.groundedDownwardBias",
                self.vertical.grounded_downward_bias,
            ),
            ("jump.bufferSeconds", self.jump.buffer_seconds),
            ("jump.coyoteSeconds", self.jump.coyote_seconds),
            (
                "jump.landingLockoutSeconds",
                self.jump.landing_lockout_seconds,
            ),
            (
                "surface.steepSlideAcceleration",
                self.surface.steep_slide_acceleration,
            ),
            ("surface.steepSlideSpeed", self.surface.steep_slide_speed),
            (
                "surface.maximumStepHeight",
                self.surface.maximum_step_height,
            ),
            ("surface.minimumStepWidth", self.surface.minimum_step_width),
            (
                "surface.floorSnapDistance",
                self.surface.floor_snap_distance,
            ),
            (
                "surface.floorSnapSpeedLimit",
                self.surface.floor_snap_speed_limit,
            ),
            ("recovery.maximumDistance", self.recovery.maximum_distance),
            ("recovery.maximumSpeed", self.recovery.maximum_speed),
            ("recovery.normalNudge", self.recovery.normal_nudge),
            (
                "recovery.unresolvedTolerance",
                self.recovery.unresolved_tolerance,
            ),
            (
                "platform.departureVelocityFactor",
                self.platform.departure_velocity_factor,
            ),
            (
                "platform.supportLossGraceSeconds",
                self.platform.support_loss_grace_seconds,
            ),
            ("platform.crushTolerance", self.platform.crush_tolerance),
            (
                "externalMotion.impulseScale",
                self.external_motion.impulse_scale,
            ),
            (
                "externalMotion.externalDecayPerSecond",
                self.external_motion.external_decay_per_second,
            ),
            (
                "externalMotion.maximumExternalSpeed",
                self.external_motion.maximum_external_speed,
            ),
            (
                "externalMotion.dynamicImpulseFactor",
                self.external_motion.dynamic_impulse_factor,
            ),
            (
                "externalMotion.maximumDynamicImpulse",
                self.external_motion.maximum_dynamic_impulse,
            ),
            (
                "solver.maximumDisplacementPerStep",
                self.solver.maximum_displacement_per_step,
            ),
        ] {
            nonnegative(field, value)?;
        }
        range(
            "ground.directionChangeMultiplier",
            self.ground.direction_change_multiplier,
            0.0,
            10.0,
        )?;
        range("air.lateralControl", self.air.lateral_control, 0.0, 10.0)?;
        range(
            "surface.maximumSlopeRadians",
            self.surface.maximum_slope_radians,
            0.0,
            std::f32::consts::FRAC_PI_2,
        )?;
        range(
            "surface.slopeHysteresisRadians",
            self.surface.slope_hysteresis_radians,
            0.0,
            0.25,
        )?;
        range(
            "surface.ledgeSupportFraction",
            self.surface.ledge_support_fraction,
            0.0,
            1.0,
        )?;
        positive(
            "externalMotion.authoredMass",
            self.external_motion.authored_mass,
        )?;
        if self.solver.maximum_slide_planes < 3
            || self.solver.maximum_cast_iterations == 0
            || self.solver.maximum_recovery_passes == 0
            || self.solver.maximum_contacts == 0
            || self.solver.maximum_step_attempts == 0
            || self.solver.maximum_queries_per_step == 0
        {
            return Err(CharacterConfigError::new("solver"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterConfigError {
    pub field: &'static str,
}

impl CharacterConfigError {
    const fn new(field: &'static str) -> Self {
        Self { field }
    }
}

impl std::fmt::Display for CharacterConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid character controller config field {}",
            self.field
        )
    }
}

impl std::error::Error for CharacterConfigError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterControllerCommand {
    pub planar_intent: Vec2,
    pub heading_yaw_radians: f32,
    pub jump_pressed: bool,
    pub jump_held: bool,
    pub crouch_requested: bool,
    pub external_velocity: Vec3,
    pub external_impulse: Vec3,
    pub step_seconds: f32,
    pub sequence: u64,
}

impl CharacterControllerCommand {
    pub const fn idle(step_seconds: f32, sequence: u64) -> Self {
        Self {
            planar_intent: Vec2::ZERO,
            heading_yaw_radians: 0.0,
            jump_pressed: false,
            jump_held: false,
            crouch_requested: false,
            external_velocity: Vec3::ZERO,
            external_impulse: Vec3::ZERO,
            step_seconds,
            sequence,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterContactKind {
    Ground,
    SteepSlope,
    Wall,
    Ceiling,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterContactFact {
    pub source: CharacterCollisionSource,
    pub point: Vec3,
    pub normal: Vec3,
    pub time_of_impact: f32,
    pub kind: CharacterContactKind,
    pub start_solid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterGroundFact {
    pub source: CharacterCollisionSource,
    pub point: Vec3,
    pub normal: Vec3,
    pub snapped_distance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterFloorProbeFact {
    pub rejected_hit: Option<CharacterContactFact>,
    pub accepted_support: Option<CharacterGroundFact>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterStanceFact {
    pub requested: CharacterStance,
    pub accepted: CharacterStance,
    pub blocked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterStepFact {
    pub attempted: bool,
    pub accepted: bool,
    pub rise: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterPlatformFact {
    pub entity: EntityId,
    pub carried_displacement: Vec3,
    pub point_velocity: Vec3,
    pub departed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicImpulseProposal {
    pub entity: EntityId,
    pub point: Vec3,
    pub impulse: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterBlockKind {
    Wall,
    Ceiling,
    SteepSlope,
    StartSolid,
    SolverBudget,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterControllerReceipt {
    pub generation: u64,
    pub revision_before: u64,
    pub revision_after: u64,
    pub entity: EntityId,
    pub command_sequence: u64,
    pub transform_before: EntityTransform,
    pub transform_after: EntityTransform,
    pub motion_before: CharacterMotionComponent,
    pub motion_after: CharacterMotionComponent,
    pub wish_velocity: Vec3,
    pub displacement: Vec3,
    pub ground: Option<CharacterGroundFact>,
    pub floor_probe: Option<CharacterFloorProbeFact>,
    pub contacts: Vec<CharacterContactFact>,
    pub blocks: Vec<CharacterBlockKind>,
    pub stance: CharacterStanceFact,
    pub step: Option<CharacterStepFact>,
    pub platform: Option<CharacterPlatformFact>,
    pub dynamic_impulses: Vec<DynamicImpulseProposal>,
    pub cast_count: u16,
    pub recovery_passes: u8,
    pub recovery_distance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterControllerReadout {
    pub generation: u64,
    pub entity: EntityId,
    pub command_sequence: u64,
    pub grounded: bool,
    pub contact_count: usize,
    pub collision_world_hash: u64,
}

#[derive(Debug)]
pub enum CharacterControllerError {
    InvalidConfig(CharacterConfigError),
    InvalidCommand,
    DisplacementEnvelopeExceeded { requested: f32, maximum: f32 },
    DuplicateOrOldCommand { previous: u64, requested: u64 },
    UnknownEntity { entity: EntityId },
    InactiveEntity { entity: EntityId },
    MissingTransform { entity: EntityId },
    MissingMotion { entity: EntityId },
    ParentedEntity { entity: EntityId },
    NonUnitScale { entity: EntityId },
    Collision(CharacterCollisionQueryError),
    Publication(CharacterMotionPublicationError),
    StaleEnvironment,
    UnresolvedPenetration { depth: f32 },
    PlatformCrush { depth: f32, tolerance: f32 },
    OutputOutOfRange,
    GenerationExhausted,
}

impl CharacterControllerError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidConfig(_) => "invalid-character-controller-config",
            Self::InvalidCommand => "invalid-character-controller-command",
            Self::DisplacementEnvelopeExceeded { .. } => {
                "character-controller-displacement-envelope-exceeded"
            }
            Self::DuplicateOrOldCommand { .. } => "stale-character-controller-command",
            Self::UnknownEntity { .. } => "unknown-character-controller-entity",
            Self::InactiveEntity { .. } => "inactive-character-controller-entity",
            Self::MissingTransform { .. } => "missing-character-controller-transform",
            Self::MissingMotion { .. } => "missing-character-motion-component",
            Self::ParentedEntity { .. } => "parented-character-controller-entity",
            Self::NonUnitScale { .. } => "scaled-character-controller-entity",
            Self::Collision(_) => "character-controller-collision-query-failed",
            Self::Publication(error) => error.code(),
            Self::StaleEnvironment => "stale-character-controller-environment",
            Self::UnresolvedPenetration { .. } => "unresolved-character-controller-penetration",
            Self::PlatformCrush { .. } => "character-controller-platform-crush",
            Self::OutputOutOfRange => "character-controller-output-out-of-range",
            Self::GenerationExhausted => "character-controller-generation-exhausted",
        }
    }
}

impl std::fmt::Display for CharacterControllerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.code())
    }
}

impl std::error::Error for CharacterControllerError {}

impl From<CharacterCollisionQueryError> for CharacterControllerError {
    fn from(value: CharacterCollisionQueryError) -> Self {
        Self::Collision(value)
    }
}
impl From<CharacterMotionPublicationError> for CharacterControllerError {
    fn from(value: CharacterMotionPublicationError) -> Self {
        Self::Publication(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CharacterEnvironmentIdentity {
    source_revision: u64,
    authority_hash: u64,
    projection_version: u64,
    static_mesh_revision: u64,
    obstacle_hash: u64,
}

#[derive(Clone)]
pub struct PreparedCharacterControllerStep {
    entity: EntityId,
    transform_revision: ComponentRevision,
    motion_revision: ComponentRevision,
    transform_before: TransformComponent,
    transform_after: TransformComponent,
    motion_before: CharacterMotionComponent,
    motion_after: CharacterMotionComponent,
    wish_velocity: Vec3,
    contacts: Vec<CharacterContactFact>,
    blocks: Vec<CharacterBlockKind>,
    ground: Option<CharacterGroundFact>,
    floor_probe: Option<CharacterFloorProbeFact>,
    stance: CharacterStanceFact,
    step: Option<CharacterStepFact>,
    platform: Option<CharacterPlatformFact>,
    dynamic_impulses: Vec<DynamicImpulseProposal>,
    cast_count: u16,
    recovery_passes: u8,
    recovery_distance: f32,
    environment: CharacterEnvironmentIdentity,
}

#[derive(Debug, Default)]
pub struct CharacterControllerService {
    generation: u64,
    last_readout: Option<CharacterControllerReadout>,
}

impl CharacterControllerService {
    pub fn readout(&self) -> Option<CharacterControllerReadout> {
        self.last_readout
    }

    pub fn step(
        &mut self,
        entities: &mut EntityState,
        scene: &VoxelCollisionScene,
        entity: EntityId,
        config: &CharacterControllerConfig,
        command: CharacterControllerCommand,
    ) -> Result<CharacterControllerReceipt, CharacterControllerError> {
        let prepared = self.prepare(entities, scene, entity, config, command)?;
        self.commit(entities, scene, prepared)
    }

    pub fn prepare(
        &self,
        entities: &EntityState,
        scene: &VoxelCollisionScene,
        entity: EntityId,
        config: &CharacterControllerConfig,
        command: CharacterControllerCommand,
    ) -> Result<PreparedCharacterControllerStep, CharacterControllerError> {
        config
            .validate()
            .map_err(CharacterControllerError::InvalidConfig)?;
        validate_command(command, config)?;
        let core = entities
            .core(entity)
            .ok_or(CharacterControllerError::UnknownEntity { entity })?;
        if core.lifecycle != EntityLifecycle::Active {
            return Err(CharacterControllerError::InactiveEntity { entity });
        }
        if entities.transform_parent(entity).is_some() {
            return Err(CharacterControllerError::ParentedEntity { entity });
        }
        let transform_before = *entities
            .transform(entity)
            .ok_or(CharacterControllerError::MissingTransform { entity })?;
        if transform_before.scale != Vec3::ONE {
            return Err(CharacterControllerError::NonUnitScale { entity });
        }
        let motion_before = *entities
            .character_motion(entity)
            .ok_or(CharacterControllerError::MissingMotion { entity })?;
        if command.sequence <= motion_before.last_command_sequence {
            return Err(CharacterControllerError::DuplicateOrOldCommand {
                previous: motion_before.last_command_sequence,
                requested: command.sequence,
            });
        }
        let transform_revision = entities
            .component_revision::<TransformComponent>(entity)
            .expect("built-in transform registration");
        let motion_revision = entities
            .component_revision::<CharacterMotionComponent>(entity)
            .expect("built-in character-motion registration");
        let obstacles = character_obstacles(entities, entity);
        let environment = character_environment(scene, &obstacles);
        let world_hash = hash_environment(environment);
        let dt = command.step_seconds;
        let mut motion = motion_before;
        if motion.collision_world_hash != 0 && motion.collision_world_hash != world_hash {
            clear_support(&mut motion);
        }
        motion.collision_world_hash = world_hash;
        motion.last_command_sequence = command.sequence;
        motion.jump_buffer_remaining = countdown(motion.jump_buffer_remaining, dt);
        motion.coyote_remaining = countdown(motion.coyote_remaining, dt);
        motion.landing_lockout_remaining = countdown(motion.landing_lockout_remaining, dt);
        if command.jump_pressed {
            motion.jump_buffer_remaining = config.jump.buffer_seconds;
        }

        let requested_stance = if command.crouch_requested {
            CharacterStance::Crouched
        } else {
            CharacterStance::Standing
        };
        let mut center = vec3_f64(transform_before.translation);
        let mut platform = apply_platform_carry(entities, &mut motion, &mut center, dt, config)?;
        let mut stance_blocked = false;
        let old_height = stance_height(config, motion.stance);
        if requested_stance != motion.stance {
            let new_height = stance_height(config, requested_stance);
            let candidate_center = WorldPos::new(
                center.x,
                center.y + f64::from((new_height - old_height) * 0.5),
                center.z,
            );
            let candidate = capsule_at(candidate_center, new_height, config.shape.radius);
            if requested_stance == CharacterStance::Standing
                && overlap_world(&scene.projection, &obstacles, candidate)?.is_some()
            {
                stance_blocked = true;
            } else {
                center = candidate_center;
                motion.stance = requested_stance;
            }
        }

        let mut cast_count = 0u16;
        let mut recovery_passes = 0u8;
        let mut recovery_distance = 0.0f32;
        let height = stance_height(config, motion.stance);
        let capsule = |center| capsule_at(center, height, config.shape.radius);
        let mut blocks = Vec::new();
        for _ in 0..config.solver.maximum_recovery_passes {
            let Some(overlap) = overlap_world(&scene.projection, &obstacles, capsule(center))?
            else {
                break;
            };
            recovery_passes += 1;
            blocks.push(CharacterBlockKind::StartSolid);
            let requested_correction =
                finite_f32(overlap.penetration_depth)? + config.recovery.normal_nudge;
            let remaining_distance = config.recovery.maximum_distance - recovery_distance;
            let correction = requested_correction
                .min(config.recovery.maximum_speed * dt)
                .min(remaining_distance);
            if correction <= 0.0 {
                break;
            }
            center = add_world(center, scale_world(overlap.normal, f64::from(correction)));
            recovery_distance += correction;
        }
        if let Some(overlap) = overlap_world(&scene.projection, &obstacles, capsule(center))? {
            let depth = finite_f32(overlap.penetration_depth)?;
            if depth > config.recovery.unresolved_tolerance {
                if platform.is_some() && depth > config.platform.crush_tolerance {
                    return Err(CharacterControllerError::PlatformCrush {
                        depth,
                        tolerance: config.platform.crush_tolerance,
                    });
                }
                return Err(CharacterControllerError::UnresolvedPenetration { depth });
            }
        }

        let input = normalized_intent(command.planar_intent);
        let wish_velocity = wish_velocity(input, command.heading_yaw_radians, &config.ground);
        let mut controlled = motion.controlled_velocity;
        if motion.grounded {
            controlled = ground_velocity(controlled, wish_velocity, config, dt);
            motion.coyote_remaining = config.jump.coyote_seconds;
        } else {
            controlled = air_velocity(controlled, wish_velocity, config, dt);
        }
        let jump_available = motion.grounded || motion.coyote_remaining > 0.0;
        let jump_requested = motion.jump_buffer_remaining > 0.0
            || (config.jump.held_input_retriggers && command.jump_held);
        if jump_available && jump_requested && motion.landing_lockout_remaining <= 0.0 {
            if config.platform.inherit_departure_velocity && motion.support_entity.is_some() {
                motion.external_velocity = motion.external_velocity
                    + motion.support_point_velocity * config.platform.departure_velocity_factor;
                if let Some(fact) = &mut platform {
                    fact.departed = true;
                }
            }
            controlled.y = config.vertical.jump_speed;
            motion.jump_buffer_remaining = 0.0;
            motion.coyote_remaining = 0.0;
            motion.grounded = false;
            clear_support(&mut motion);
        } else if !motion.grounded || controlled.y > 0.0 {
            controlled.y = (controlled.y - config.vertical.gravity * dt).clamp(
                -config.vertical.terminal_fall_speed,
                config.vertical.terminal_rise_speed,
            );
        } else {
            controlled.y = -config.vertical.grounded_downward_bias;
        }
        motion.external_velocity = motion.external_velocity
            + command.external_impulse * config.external_motion.impulse_scale;
        if command.external_velocity != Vec3::ZERO {
            motion.external_velocity = command.external_velocity;
        }
        let external_speed = motion.external_velocity.length();
        if external_speed > 0.0 {
            let next_speed = (external_speed
                - config.external_motion.external_decay_per_second * dt)
                .max(0.0)
                .min(config.external_motion.maximum_external_speed);
            motion.external_velocity = motion.external_velocity * (next_speed / external_speed);
        }
        let total_velocity = controlled + motion.external_velocity;
        let mut requested = total_velocity * dt;
        if motion.grounded && requested.y < 0.0 {
            requested.y = 0.0;
        }
        let requested_distance = requested.length();
        if requested_distance > config.solver.maximum_displacement_per_step {
            return Err(CharacterControllerError::DisplacementEnvelopeExceeded {
                requested: requested_distance,
                maximum: config.solver.maximum_displacement_per_step,
            });
        }
        let solve = move_and_slide(MoveSolveInput {
            projection: &scene.projection,
            obstacles: &obstacles,
            center,
            capsule: &capsule,
            requested,
            velocity: total_velocity,
            may_step: motion.grounded,
            config,
        })?;
        cast_count = cast_count.saturating_add(solve.casts);
        center = solve.center;
        blocks.extend(solve.blocks);
        controlled = solve.velocity - motion.external_velocity;
        let contacts = solve.contacts;
        let step = solve.step;

        if let Some(contact) = contacts
            .iter()
            .find(|contact| contact.kind == CharacterContactKind::SteepSlope)
        {
            let down = Vec3::new(0.0, -1.0, 0.0);
            let tangent = down - contact.normal * down.dot(contact.normal);
            let length = tangent.length();
            if length > 1.0e-5 {
                let downhill = tangent * (1.0 / length);
                controlled = clamp_length(
                    controlled + downhill * config.surface.steep_slide_acceleration * dt,
                    config.surface.steep_slide_speed,
                );
            }
        }

        let mut ground = contacts
            .iter()
            .rev()
            .find(|contact| contact.kind == CharacterContactKind::Ground)
            .map(|contact| CharacterGroundFact {
                source: contact.source,
                point: contact.point,
                normal: contact.normal,
                snapped_distance: 0.0,
            });
        let mut floor_probe = None;
        if controlled.y <= 0.0
            && controlled.y.abs() <= config.surface.floor_snap_speed_limit
            && ground.is_none()
            && config.surface.floor_snap_distance > 0.0
            && cast_count < config.solver.maximum_queries_per_step
        {
            cast_count = cast_count.saturating_add(1);
            let broad_hit = cast_world(
                &scene.projection,
                &obstacles,
                capsule(center),
                WorldVec::new(0.0, -f64::from(config.surface.floor_snap_distance), 0.0),
                f64::from(config.shape.contact_skin),
            )?;
            let mut rejected_hit = None;
            let mut accepted_support = None;
            if let Some(hit) = broad_hit {
                let normal = vec3_from_world(hit.normal)?;
                if standable(normal, config) {
                    let snap = config.surface.floor_snap_distance * finite_f32(hit.time_of_impact)?;
                    center.y -= f64::from(snap);
                    controlled.y = 0.0;
                    accepted_support = Some(CharacterGroundFact {
                        source: hit.source,
                        point: vec3_from_pos(hit.point)?,
                        normal,
                        snapped_distance: snap,
                    });
                } else {
                    rejected_hit = Some(contact_fact(
                        hit,
                        normal,
                        contact_kind(normal, config.surface.maximum_slope_radians.cos()),
                    )?);
                    if cast_count < config.solver.maximum_queries_per_step {
                        cast_count = cast_count.saturating_add(1);
                        if let Some(support) = cast_world(
                            &scene.projection,
                            &obstacles,
                            bounded_support_probe(capsule(center), config),
                            WorldVec::new(0.0, -f64::from(config.surface.floor_snap_distance), 0.0),
                            0.0,
                        )? {
                            let support_normal = vec3_from_world(support.normal)?;
                            if standable(support_normal, config) {
                                let snap = config.surface.floor_snap_distance
                                    * finite_f32(support.time_of_impact)?;
                                center.y -= f64::from(snap);
                                controlled.y = 0.0;
                                accepted_support = Some(CharacterGroundFact {
                                    source: support.source,
                                    point: vec3_from_pos(support.point)?,
                                    normal: support_normal,
                                    snapped_distance: snap,
                                });
                            }
                        }
                    }
                }
            }
            ground = accepted_support;
            floor_probe = Some(CharacterFloorProbeFact {
                rejected_hit,
                accepted_support,
            });
        }
        motion.grounded = ground.is_some();
        if motion.grounded {
            motion.coyote_remaining = config.jump.coyote_seconds;
            if !motion_before.grounded {
                motion.landing_lockout_remaining = config.jump.landing_lockout_seconds;
            }
        }
        update_platform_support(entities, &mut motion, &ground, &mut platform, dt, config)?;
        let dynamic_impulses = dynamic_impulse_proposals(
            entities,
            &contacts,
            controlled + motion.external_velocity,
            config,
        );
        motion.controlled_velocity = controlled;
        if center.y as f32 > motion.peak_y {
            motion.peak_y = center.y as f32;
        }
        if motion.grounded {
            motion.fall_origin_y = center.y as f32;
            motion.peak_y = center.y as f32;
        }
        let translation = vec3_from_pos(center)?;
        let transform_after = TransformComponent {
            translation,
            ..transform_before
        };
        Ok(PreparedCharacterControllerStep {
            entity,
            transform_revision,
            motion_revision,
            transform_before,
            transform_after,
            motion_before,
            motion_after: motion,
            wish_velocity,
            contacts,
            blocks,
            ground,
            floor_probe,
            stance: CharacterStanceFact {
                requested: requested_stance,
                accepted: motion.stance,
                blocked: stance_blocked,
            },
            step,
            platform,
            dynamic_impulses,
            cast_count,
            recovery_passes,
            recovery_distance,
            environment,
        })
    }

    pub fn commit(
        &mut self,
        entities: &mut EntityState,
        scene: &VoxelCollisionScene,
        prepared: PreparedCharacterControllerStep,
    ) -> Result<CharacterControllerReceipt, CharacterControllerError> {
        let obstacles = character_obstacles(entities, prepared.entity);
        if character_environment(scene, &obstacles) != prepared.environment {
            return Err(CharacterControllerError::StaleEnvironment);
        }
        let publication = replace_character_motion_state(
            entities,
            CharacterMotionStateReplacement {
                entity: prepared.entity,
                expected_transform_revision: prepared.transform_revision,
                expected_motion_revision: prepared.motion_revision,
                transform: prepared.transform_after,
                motion: prepared.motion_after,
            },
        )?;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(CharacterControllerError::GenerationExhausted)?;
        let displacement =
            prepared.transform_after.translation - prepared.transform_before.translation;
        self.last_readout = Some(CharacterControllerReadout {
            generation: self.generation,
            entity: prepared.entity,
            command_sequence: prepared.motion_after.last_command_sequence,
            grounded: prepared.motion_after.grounded,
            contact_count: prepared.contacts.len(),
            collision_world_hash: prepared.motion_after.collision_world_hash,
        });
        Ok(CharacterControllerReceipt {
            generation: self.generation,
            revision_before: publication.revision_before,
            revision_after: publication.revision_after,
            entity: prepared.entity,
            command_sequence: prepared.motion_after.last_command_sequence,
            transform_before: prepared.transform_before.transform(),
            transform_after: prepared.transform_after.transform(),
            motion_before: prepared.motion_before,
            motion_after: prepared.motion_after,
            wish_velocity: prepared.wish_velocity,
            displacement,
            ground: prepared.ground,
            floor_probe: prepared.floor_probe,
            contacts: prepared.contacts,
            blocks: prepared.blocks,
            stance: prepared.stance,
            step: prepared.step,
            platform: prepared.platform,
            dynamic_impulses: prepared.dynamic_impulses,
            cast_count: prepared.cast_count,
            recovery_passes: prepared.recovery_passes,
            recovery_distance: prepared.recovery_distance,
        })
    }
}

struct MoveSolveInput<'a, F> {
    projection: &'a svc_collision::CollisionProjection,
    obstacles: &'a [CharacterObstacle],
    center: WorldPos,
    capsule: &'a F,
    requested: Vec3,
    velocity: Vec3,
    may_step: bool,
    config: &'a CharacterControllerConfig,
}

struct MoveSolveOutput {
    center: WorldPos,
    velocity: Vec3,
    contacts: Vec<CharacterContactFact>,
    blocks: Vec<CharacterBlockKind>,
    step: Option<CharacterStepFact>,
    casts: u16,
}

fn move_and_slide<F>(
    input: MoveSolveInput<'_, F>,
) -> Result<MoveSolveOutput, CharacterControllerError>
where
    F: Fn(WorldPos) -> CharacterCapsule,
{
    let MoveSolveInput {
        projection,
        obstacles,
        mut center,
        capsule,
        requested,
        mut velocity,
        may_step,
        config,
    } = input;
    let mut remaining = requested;
    let mut contacts: Vec<CharacterContactFact> = Vec::new();
    let mut blocks = Vec::new();
    let mut casts = 0u16;
    let mut planes = Vec::new();
    let mut step_attempted = false;
    let mut query_offset = Vec3::ZERO;
    let requested_start = requested;
    let start_center = center;
    let slope_limit = config.surface.maximum_slope_radians
        + if may_step {
            config.surface.slope_hysteresis_radians
        } else {
            0.0
        };
    let slope_cos = slope_limit.min(std::f32::consts::FRAC_PI_2).cos();
    let cast_budget = u16::from(config.solver.maximum_cast_iterations)
        .min(config.solver.maximum_queries_per_step);
    for _ in 0..cast_budget {
        if remaining.length_squared() <= 1.0e-10 {
            break;
        }
        casts = casts.saturating_add(1);
        let Some(hit) = cast_world(
            projection,
            obstacles,
            capsule(add_world(center, vec3_world(query_offset))),
            vec3_world(remaining),
            f64::from(config.shape.contact_skin),
        )?
        else {
            center = add_world(center, vec3_world(remaining));
            remaining = Vec3::ZERO;
            break;
        };
        let toi = finite_f32(hit.time_of_impact)?.clamp(0.0, 1.0);
        center = add_world(center, vec3_world(remaining * toi));
        let normal = vec3_from_world(hit.normal)?;
        let kind = contact_kind(normal, slope_cos);
        let solve_normal = if kind == CharacterContactKind::SteepSlope {
            let horizontal = Vec3::new(normal.x, 0.0, normal.z);
            let length = horizontal.length();
            if length > 1.0e-5 {
                horizontal * (1.0 / length)
            } else {
                normal
            }
        } else {
            normal
        };
        if may_step
            && config.solver.maximum_step_attempts > 0
            && casts.saturating_add(4) <= config.solver.maximum_queries_per_step
            && contacts
                .iter()
                .all(|contact| contact.kind == CharacterContactKind::Ground)
            && matches!(
                kind,
                CharacterContactKind::Wall | CharacterContactKind::SteepSlope
            )
        {
            step_attempted = true;
            if let Some((stepped_center, step_rise, step_casts)) = try_step(
                projection,
                obstacles,
                start_center,
                &capsule,
                requested_start,
                config,
            )? {
                casts = casts.saturating_add(step_casts);
                return Ok(MoveSolveOutput {
                    center: stepped_center,
                    velocity,
                    contacts,
                    blocks,
                    step: Some(CharacterStepFact {
                        attempted: true,
                        accepted: true,
                        rise: step_rise,
                    }),
                    casts,
                });
            }
        }
        if let Some(block) = match kind {
            CharacterContactKind::Ground => None,
            CharacterContactKind::SteepSlope => Some(CharacterBlockKind::SteepSlope),
            CharacterContactKind::Wall => Some(CharacterBlockKind::Wall),
            CharacterContactKind::Ceiling => Some(CharacterBlockKind::Ceiling),
        } {
            blocks.push(block);
        }
        if contacts.len() < usize::from(config.solver.maximum_contacts) {
            contacts.push(contact_fact(hit, normal, kind)?);
        }
        if planes.len() < usize::from(config.solver.maximum_slide_planes)
            && !planes
                .iter()
                .any(|plane: &Vec3| plane.dot(solve_normal) > 0.999)
        {
            planes.push(solve_normal);
        }
        let remainder = remaining * (1.0 - toi);
        remaining = clip_against_planes(remainder, &planes);
        velocity = clip_against_planes(velocity, &planes);
        if hit.start_solid {
            query_offset =
                query_offset + normal * (config.shape.contact_skin + config.recovery.normal_nudge);
        }
    }
    if remaining.length_squared() > 1.0e-8 {
        blocks.push(CharacterBlockKind::SolverBudget);
    }
    Ok(MoveSolveOutput {
        center,
        velocity,
        contacts,
        blocks,
        step: step_attempted.then_some(CharacterStepFact {
            attempted: true,
            accepted: false,
            rise: 0.0,
        }),
        casts,
    })
}

fn try_step(
    projection: &svc_collision::CollisionProjection,
    obstacles: &[CharacterObstacle],
    start: WorldPos,
    capsule: &impl Fn(WorldPos) -> CharacterCapsule,
    requested: Vec3,
    config: &CharacterControllerConfig,
) -> Result<Option<(WorldPos, f32, u16)>, CharacterControllerError> {
    let rise = config.surface.maximum_step_height;
    if rise <= 0.0 || requested.x * requested.x + requested.z * requested.z <= 1.0e-8 {
        return Ok(None);
    }
    // A grounded capsule can be within the query skin of its support. Begin the
    // upward clearance cast just beyond that skin so the separating floor does
    // not mask a real ceiling farther along the probe.
    let departure = (config.shape.contact_skin + config.recovery.normal_nudge).min(rise);
    let upward_start = add_world(start, WorldVec::new(0.0, f64::from(departure), 0.0));
    let upward = WorldVec::new(0.0, f64::from(rise - departure), 0.0);
    if cast_world(
        projection,
        obstacles,
        capsule(upward_start),
        upward,
        f64::from(config.shape.contact_skin),
    )?
    .is_some()
    {
        return Ok(None);
    }
    let raised = add_world(start, WorldVec::new(0.0, f64::from(rise), 0.0));
    let horizontal = Vec3::new(requested.x, 0.0, requested.z);
    if cast_world(
        projection,
        obstacles,
        capsule(raised),
        vec3_world(horizontal),
        f64::from(config.shape.contact_skin),
    )?
    .is_some()
    {
        return Ok(None);
    }
    let forward = add_world(raised, vec3_world(horizontal));
    let downward_distance = rise + config.surface.floor_snap_distance;
    let Some(landing) = cast_world(
        projection,
        obstacles,
        capsule(forward),
        WorldVec::new(0.0, -f64::from(downward_distance), 0.0),
        f64::from(config.shape.contact_skin),
    )?
    else {
        return Ok(None);
    };
    // The full capsule can first touch a top edge with a diagonal cap normal.
    // Confirm support with a narrow bounded probe at the accepted horizontal
    // endpoint so edge geometry cannot be mistaken for an over-limit slope.
    let support_probe = bounded_support_probe(capsule(forward), config);
    let Some(support) = cast_world(
        projection,
        obstacles,
        support_probe,
        WorldVec::new(0.0, -f64::from(downward_distance), 0.0),
        0.0,
    )?
    else {
        return Ok(None);
    };
    if !standable(vec3_from_world(support.normal)?, config) {
        return Ok(None);
    }
    let drop = downward_distance * finite_f32(landing.time_of_impact)?;
    let support_drop = downward_distance * finite_f32(support.time_of_impact)?;
    // The broad capsule may meet a rounded top edge before the central support probe. Permit an
    // edge normal beyond the ordinary slope limit only by the angle implied by the minimum
    // accepted tread width over the capsule radius. A more wall-dominant hit combined with a
    // different support height describes two surfaces; accepting its broad height would
    // manufacture an up-step toward adjacent terrain while actual support remains below.
    let landing_height_tolerance = config.shape.contact_skin
        + config
            .surface
            .minimum_step_width
            .max(config.recovery.normal_nudge);
    let landing_normal = vec3_from_world(landing.normal)?;
    let edge_angle_allowance = (config.surface.minimum_step_width / config.shape.radius).atan();
    let minimum_edge_normal_y = (config.surface.maximum_slope_radians + edge_angle_allowance)
        .min(std::f32::consts::FRAC_PI_2)
        .cos();
    if landing_normal.y < minimum_edge_normal_y
        && (drop - support_drop).abs() > landing_height_tolerance
    {
        return Ok(None);
    }
    let landed = WorldPos::new(forward.x, forward.y - f64::from(drop), forward.z);
    let actual_rise = (landed.y - start.y).max(0.0) as f32;
    Ok(Some((landed, actual_rise, 4)))
}

fn contact_fact(
    hit: CharacterCapsuleCastHit,
    normal: Vec3,
    kind: CharacterContactKind,
) -> Result<CharacterContactFact, CharacterControllerError> {
    Ok(CharacterContactFact {
        source: hit.source,
        point: vec3_from_pos(hit.point)?,
        normal,
        time_of_impact: finite_f32(hit.time_of_impact)?,
        kind,
        start_solid: hit.start_solid,
    })
}

fn normalized_intent(input: Vec2) -> Vec2 {
    let length = input.length();
    if !length.is_finite() || length <= 1.0 {
        input
    } else {
        input * (1.0 / length)
    }
}

fn wish_velocity(input: Vec2, yaw: f32, config: &CharacterGroundConfig) -> Vec3 {
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let right = Vec3::new(cos_yaw, 0.0, sin_yaw);
    let forward = Vec3::new(sin_yaw, 0.0, -cos_yaw);
    let forward_speed = if input.y >= 0.0 {
        config.forward_speed
    } else {
        config.backward_speed
    };
    right * (input.x * config.strafe_speed) + forward * (input.y * forward_speed)
}

fn ground_velocity(
    mut current: Vec3,
    wish: Vec3,
    config: &CharacterControllerConfig,
    dt: f32,
) -> Vec3 {
    let planar = Vec3::new(current.x, 0.0, current.z);
    let speed = planar.length();
    if speed > 0.0 {
        let drop = config.ground.friction * speed.max(config.ground.stop_speed) * dt;
        let retained = ((speed - drop).max(0.0) / speed).min(1.0);
        current.x *= retained;
        current.z *= retained;
    }
    let direction_change = if planar.dot(wish) < 0.0 {
        config.ground.direction_change_multiplier
    } else {
        1.0
    };
    accelerate(
        current,
        wish,
        config.ground.acceleration * direction_change,
        config.ground.braking,
        dt,
    )
}

fn air_velocity(current: Vec3, wish: Vec3, config: &CharacterControllerConfig, dt: f32) -> Vec3 {
    let wish = clamp_planar(wish, config.air.wish_speed_cap);
    let mut next = accelerate(
        current,
        wish,
        config.air.acceleration * config.air.lateral_control,
        config.air.braking,
        dt,
    );
    let drag = (1.0 - config.air.drag * dt).max(0.0);
    next.x *= drag;
    next.z *= drag;
    let planar = Vec3::new(next.x, 0.0, next.z);
    let speed = planar.length();
    if speed > config.air.maximum_speed && speed > 0.0 {
        let scale = config.air.maximum_speed / speed;
        next.x *= scale;
        next.z *= scale;
    }
    next
}

fn accelerate(mut current: Vec3, wish: Vec3, acceleration: f32, braking: f32, dt: f32) -> Vec3 {
    let speed = wish.length();
    if speed <= 0.0 {
        let planar = Vec3::new(current.x, 0.0, current.z);
        let length = planar.length();
        if length > 0.0 && braking > 0.0 {
            let retained = ((length - braking * dt).max(0.0) / length).min(1.0);
            current.x *= retained;
            current.z *= retained;
        }
        return current;
    }
    let direction = wish * (1.0 / speed);
    let current_along = current.dot(direction);
    let add = (speed - current_along).clamp(0.0, acceleration * speed * dt);
    current + direction * add
}

fn clamp_planar(value: Vec3, maximum: f32) -> Vec3 {
    let length = Vec3::new(value.x, 0.0, value.z).length();
    if length > maximum && length > 0.0 {
        value * (maximum / length)
    } else {
        value
    }
}

fn clamp_length(value: Vec3, maximum: f32) -> Vec3 {
    let length = value.length();
    if length > maximum && length > 0.0 {
        value * (maximum / length)
    } else {
        value
    }
}

fn remove_inward(value: Vec3, normal: Vec3) -> Vec3 {
    let inward = value.dot(normal);
    if inward < 0.0 {
        value - normal * inward
    } else {
        value
    }
}

fn clip_against_planes(mut value: Vec3, planes: &[Vec3]) -> Vec3 {
    for normal in planes {
        value = remove_inward(value, *normal);
    }
    if planes.len() >= 2 {
        let crease = planes[0].cross(planes[1]);
        let length = crease.length();
        if length > 1.0e-5 {
            let direction = crease * (1.0 / length);
            value = direction * value.dot(direction);
        }
    }
    if planes.len() >= 3 {
        Vec3::ZERO
    } else {
        value
    }
}

fn standable(normal: Vec3, config: &CharacterControllerConfig) -> bool {
    normal.y >= config.surface.maximum_slope_radians.cos()
}

fn contact_kind(normal: Vec3, slope_cos: f32) -> CharacterContactKind {
    if normal.y >= slope_cos {
        CharacterContactKind::Ground
    } else if normal.y <= -0.5 {
        CharacterContactKind::Ceiling
    } else if normal.y > 0.01 {
        CharacterContactKind::SteepSlope
    } else {
        CharacterContactKind::Wall
    }
}

fn bounded_support_probe(
    full_capsule: CharacterCapsule,
    config: &CharacterControllerConfig,
) -> CharacterCapsule {
    let support_radius = f64::from(
        (config.shape.radius * config.surface.ledge_support_fraction.sqrt())
            .max(config.surface.minimum_step_width)
            .min(config.shape.radius),
    );
    CharacterCapsule {
        center: WorldPos::new(
            full_capsule.center.x,
            full_capsule.center.y - full_capsule.half_height - full_capsule.radius + support_radius,
            full_capsule.center.z,
        ),
        half_height: 0.0,
        radius: support_radius,
    }
}

fn capsule_at(center: WorldPos, total_height: f32, radius: f32) -> CharacterCapsule {
    CharacterCapsule {
        center,
        half_height: f64::from((total_height * 0.5 - radius).max(0.0)),
        radius: f64::from(radius),
    }
}

fn stance_height(config: &CharacterControllerConfig, stance: CharacterStance) -> f32 {
    match stance {
        CharacterStance::Standing => config.shape.standing_height,
        CharacterStance::Crouched => config.shape.crouched_height,
    }
}

fn validate_command(
    command: CharacterControllerCommand,
    config: &CharacterControllerConfig,
) -> Result<(), CharacterControllerError> {
    let vectors = [
        command.planar_intent.x,
        command.planar_intent.y,
        command.heading_yaw_radians,
        command.external_velocity.x,
        command.external_velocity.y,
        command.external_velocity.z,
        command.external_impulse.x,
        command.external_impulse.y,
        command.external_impulse.z,
        command.step_seconds,
    ];
    if !vectors.into_iter().all(f32::is_finite)
        || command.planar_intent.x.abs() > 1.0
        || command.planar_intent.y.abs() > 1.0
        || !(0.001..=1.0 / 15.0).contains(&command.step_seconds)
        || (command.external_velocity + command.external_impulse).length()
            > config.solver.maximum_displacement_per_step / command.step_seconds
    {
        Err(CharacterControllerError::InvalidCommand)
    } else {
        Ok(())
    }
}

fn character_environment(
    scene: &VoxelCollisionScene,
    obstacles: &[CharacterObstacle],
) -> CharacterEnvironmentIdentity {
    CharacterEnvironmentIdentity {
        source_revision: scene.source_revision().raw(),
        authority_hash: scene.authority_hash(),
        projection_version: scene.projection_version(),
        static_mesh_revision: scene.static_mesh_collision_revision(),
        obstacle_hash: hash_obstacles(obstacles),
    }
}

fn hash_environment(value: CharacterEnvironmentIdentity) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for value in [
        value.source_revision,
        value.authority_hash,
        value.projection_version,
        value.static_mesh_revision,
    ] {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

fn character_obstacles(entities: &EntityState, controlled: EntityId) -> Vec<CharacterObstacle> {
    crate::active_collision::active_entity_colliders(entities)
        .filter(|collider| collider.entity != controlled)
        .map(|collider| {
            let center = (collider.bounds.min + collider.bounds.max) * 0.5;
            let half = (collider.bounds.max - collider.bounds.min) * 0.5;
            let (linear, angular) = entities
                .rigid_body(collider.entity)
                .map(|body| (body.linear_velocity, body.angular_velocity))
                .or_else(|| {
                    entities.character_motion(collider.entity).map(|motion| {
                        (
                            motion.controlled_velocity + motion.external_velocity,
                            Vec3::ZERO,
                        )
                    })
                })
                .unwrap_or((Vec3::ZERO, Vec3::ZERO));
            CharacterObstacle {
                id: collider.entity.raw(),
                center: vec3_f64(center),
                half_extents: vec3_world(half),
                linear_velocity: vec3_world(linear),
                angular_velocity: vec3_world(angular),
            }
        })
        .collect()
}

fn hash_obstacles(obstacles: &[CharacterObstacle]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for obstacle in obstacles {
        for value in [
            obstacle.id,
            obstacle.center.x.to_bits(),
            obstacle.center.y.to_bits(),
            obstacle.center.z.to_bits(),
            obstacle.half_extents.x.to_bits(),
            obstacle.half_extents.y.to_bits(),
            obstacle.half_extents.z.to_bits(),
            obstacle.linear_velocity.x.to_bits(),
            obstacle.linear_velocity.y.to_bits(),
            obstacle.linear_velocity.z.to_bits(),
            obstacle.angular_velocity.x.to_bits(),
            obstacle.angular_velocity.y.to_bits(),
            obstacle.angular_velocity.z.to_bits(),
        ] {
            for byte in value.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
    }
    hash
}

fn cast_world(
    projection: &svc_collision::CollisionProjection,
    obstacles: &[CharacterObstacle],
    capsule: CharacterCapsule,
    translation: WorldVec,
    skin: f64,
) -> Result<Option<CharacterCapsuleCastHit>, CharacterControllerError> {
    let world = projection.cast_character_capsule(capsule, translation, skin)?;
    let entities = cast_character_capsule_against_obstacles(capsule, translation, skin, obstacles)?;
    Ok(match (world, entities) {
        (Some(world), Some(entity)) if entity.time_of_impact < world.time_of_impact => Some(entity),
        (Some(world), Some(_)) => Some(world),
        (Some(hit), None) | (None, Some(hit)) => Some(hit),
        (None, None) => None,
    })
}

fn overlap_world(
    projection: &svc_collision::CollisionProjection,
    obstacles: &[CharacterObstacle],
    capsule: CharacterCapsule,
) -> Result<Option<CharacterCapsuleOverlap>, CharacterControllerError> {
    let world = projection.character_capsule_overlap(capsule)?;
    let entities = character_capsule_overlap_obstacles(capsule, obstacles)?;
    Ok(match (world, entities) {
        (Some(world), Some(entity)) if entity.penetration_depth > world.penetration_depth => {
            Some(entity)
        }
        (Some(world), Some(_)) => Some(world),
        (Some(hit), None) | (None, Some(hit)) => Some(hit),
        (None, None) => None,
    })
}

fn apply_platform_carry(
    entities: &EntityState,
    motion: &mut CharacterMotionComponent,
    center: &mut WorldPos,
    dt: f32,
    config: &CharacterControllerConfig,
) -> Result<Option<CharacterPlatformFact>, CharacterControllerError> {
    let Some(entity) = motion.support_entity else {
        return Ok(None);
    };
    let Some(current) = entities.world_transform(entity) else {
        if config.platform.inherit_departure_velocity {
            motion.external_velocity = motion.external_velocity
                + motion.support_point_velocity * config.platform.departure_velocity_factor;
        }
        let fact = CharacterPlatformFact {
            entity,
            carried_displacement: Vec3::ZERO,
            point_velocity: motion.support_point_velocity,
            departed: true,
        };
        clear_support(motion);
        return Ok(Some(fact));
    };
    let previous = EntityTransform {
        translation: motion.support_previous_translation,
        rotation: motion.support_previous_rotation,
        scale: Vec3::ONE,
    };
    let previous_point = previous.transform_point(motion.support_local_anchor);
    let current_point = current.transform_point(motion.support_local_anchor);
    let translation_carry = if config.platform.carry_translation {
        current.translation - previous.translation
    } else {
        Vec3::ZERO
    };
    let rotation_carry = if config.platform.carry_rotation {
        (current_point - current.translation) - (previous_point - previous.translation)
    } else {
        Vec3::ZERO
    };
    let carry = translation_carry + rotation_carry;
    let physical_point_velocity = (current_point - previous_point) * (1.0 / dt);
    *center = add_world(*center, vec3_world(carry));
    motion.support_point_velocity = physical_point_velocity;
    motion.support_previous_translation = current.translation;
    motion.support_previous_rotation = current.rotation;
    Ok(Some(CharacterPlatformFact {
        entity,
        carried_displacement: carry,
        point_velocity: motion.support_point_velocity,
        departed: false,
    }))
}

fn update_platform_support(
    entities: &EntityState,
    motion: &mut CharacterMotionComponent,
    ground: &Option<CharacterGroundFact>,
    platform: &mut Option<CharacterPlatformFact>,
    _dt: f32,
    config: &CharacterControllerConfig,
) -> Result<(), CharacterControllerError> {
    let next = ground.as_ref().and_then(|ground| match ground.source {
        CharacterCollisionSource::ActiveEntity(raw) => Some((EntityId::new(raw), ground.point)),
        _ => None,
    });
    if let Some((entity, point)) = next {
        let transform = entities
            .world_transform(entity)
            .ok_or(CharacterControllerError::UnknownEntity { entity })?;
        motion.support_entity = Some(entity);
        motion.support_local_anchor =
            inverse_rotate(transform.rotation, point - transform.translation);
        motion.support_previous_translation = transform.translation;
        motion.support_previous_rotation = transform.rotation;
        let linear = entities
            .rigid_body(entity)
            .map_or(Vec3::ZERO, |body| body.linear_velocity);
        motion.support_point_velocity = linear;
        motion.coyote_remaining = motion
            .coyote_remaining
            .max(config.platform.support_loss_grace_seconds);
        if platform.is_none() {
            *platform = Some(CharacterPlatformFact {
                entity,
                carried_displacement: Vec3::ZERO,
                point_velocity: linear,
                departed: false,
            });
        }
    } else if let Some(entity) = motion.support_entity {
        if config.platform.support_loss_grace_seconds > 0.0 && motion.coyote_remaining > 0.0 {
            return Ok(());
        }
        if config.platform.inherit_departure_velocity {
            motion.external_velocity = motion.external_velocity
                + motion.support_point_velocity * config.platform.departure_velocity_factor;
        }
        *platform = Some(CharacterPlatformFact {
            entity,
            carried_displacement: platform
                .as_ref()
                .map_or(Vec3::ZERO, |fact| fact.carried_displacement),
            point_velocity: motion.support_point_velocity,
            departed: true,
        });
        clear_support(motion);
    }
    Ok(())
}

fn dynamic_impulse_proposals(
    entities: &EntityState,
    contacts: &[CharacterContactFact],
    velocity: Vec3,
    config: &CharacterControllerConfig,
) -> Vec<DynamicImpulseProposal> {
    contacts
        .iter()
        .filter_map(|contact| {
            let CharacterCollisionSource::ActiveEntity(raw) = contact.source else {
                return None;
            };
            let entity = EntityId::new(raw);
            entities.rigid_body(entity)?;
            let closing = (-velocity.dot(contact.normal)).max(0.0);
            if closing <= 0.0 {
                return None;
            }
            let magnitude = (closing
                * config.external_motion.authored_mass
                * config.external_motion.dynamic_impulse_factor)
                .min(config.external_motion.maximum_dynamic_impulse);
            Some(DynamicImpulseProposal {
                entity,
                point: contact.point,
                impulse: contact.normal * -magnitude,
            })
        })
        .collect()
}

fn inverse_rotate(rotation: Quat, vector: Vec3) -> Vec3 {
    let inverse_axis = Vec3::new(-rotation.x, -rotation.y, -rotation.z);
    let twice_cross = inverse_axis.cross(vector) * 2.0;
    vector + twice_cross * rotation.w + inverse_axis.cross(twice_cross)
}

fn clear_support(motion: &mut CharacterMotionComponent) {
    motion.support_entity = None;
    motion.support_local_anchor = Vec3::ZERO;
    motion.support_previous_translation = Vec3::ZERO;
    motion.support_previous_rotation = Quat::IDENTITY;
    motion.support_point_velocity = Vec3::ZERO;
}

fn countdown(value: f32, dt: f32) -> f32 {
    (value - dt).max(0.0)
}
fn vec3_f64(value: Vec3) -> WorldPos {
    WorldPos::new(f64::from(value.x), f64::from(value.y), f64::from(value.z))
}
fn vec3_world(value: Vec3) -> WorldVec {
    WorldVec::new(f64::from(value.x), f64::from(value.y), f64::from(value.z))
}
fn add_world(point: WorldPos, vector: WorldVec) -> WorldPos {
    WorldPos::new(point.x + vector.x, point.y + vector.y, point.z + vector.z)
}
fn scale_world(value: WorldVec, scalar: f64) -> WorldVec {
    WorldVec::new(value.x * scalar, value.y * scalar, value.z * scalar)
}
fn finite_f32(value: f64) -> Result<f32, CharacterControllerError> {
    let value = value as f32;
    value
        .is_finite()
        .then_some(value)
        .ok_or(CharacterControllerError::OutputOutOfRange)
}
fn vec3_from_world(value: WorldVec) -> Result<Vec3, CharacterControllerError> {
    Ok(Vec3::new(
        finite_f32(value.x)?,
        finite_f32(value.y)?,
        finite_f32(value.z)?,
    ))
}
fn vec3_from_pos(value: WorldPos) -> Result<Vec3, CharacterControllerError> {
    Ok(Vec3::new(
        finite_f32(value.x)?,
        finite_f32(value.y)?,
        finite_f32(value.z)?,
    ))
}

defaulted_config!(FirstPersonLookConfig {
    horizontal_radians_per_unit: f32 = 1.0,
    vertical_radians_per_unit: f32 = 1.0,
    invert_horizontal: bool = false,
    invert_vertical: bool = false,
    minimum_pitch_radians: f32 = -std::f32::consts::FRAC_PI_2 + PITCH_EPSILON,
    maximum_pitch_radians: f32 = std::f32::consts::FRAC_PI_2 - PITCH_EPSILON,
    wrap_yaw: bool = true,
    maximum_delta_radians: f32 = std::f32::consts::PI,
});

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FirstPersonLookState {
    pub yaw_radians: f32,
    pub pitch_radians: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FirstPersonLookCommand {
    pub delta: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstPersonLookReceipt {
    pub before: FirstPersonLookState,
    pub after: FirstPersonLookState,
    pub orientation: Quat,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstPersonLookError {
    InvalidConfig,
    InvalidState,
    InvalidCommand,
    DeltaLimitExceeded,
}

impl std::fmt::Display for FirstPersonLookError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "first-person look rejected: {self:?}")
    }
}

impl std::error::Error for FirstPersonLookError {}

#[derive(Debug, Clone, Copy, Default)]
pub struct FirstPersonLookService;

impl FirstPersonLookService {
    pub fn integrate(
        self,
        config: &FirstPersonLookConfig,
        state: FirstPersonLookState,
        command: FirstPersonLookCommand,
    ) -> Result<FirstPersonLookReceipt, FirstPersonLookError> {
        validate_look_config(config)?;
        if !state.yaw_radians.is_finite() || !state.pitch_radians.is_finite() {
            return Err(FirstPersonLookError::InvalidState);
        }
        if !command.delta.x.is_finite() || !command.delta.y.is_finite() {
            return Err(FirstPersonLookError::InvalidCommand);
        }
        let yaw_delta = command.delta.x
            * config.horizontal_radians_per_unit
            * if config.invert_horizontal { -1.0 } else { 1.0 };
        let pitch_delta = command.delta.y
            * config.vertical_radians_per_unit
            * if config.invert_vertical { -1.0 } else { 1.0 };
        if yaw_delta.abs() > config.maximum_delta_radians
            || pitch_delta.abs() > config.maximum_delta_radians
        {
            return Err(FirstPersonLookError::DeltaLimitExceeded);
        }
        let mut yaw = state.yaw_radians + yaw_delta;
        if config.wrap_yaw {
            yaw = wrap_radians(yaw);
        }
        let pitch = (state.pitch_radians + pitch_delta)
            .clamp(config.minimum_pitch_radians, config.maximum_pitch_radians);
        let (sin_yaw, cos_yaw) = yaw.sin_cos();
        let (sin_pitch, cos_pitch) = pitch.sin_cos();
        let forward = Vec3::new(sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch);
        let right = Vec3::new(cos_yaw, 0.0, sin_yaw);
        let up = right.cross(forward);
        let (sy, cy) = (yaw * 0.5).sin_cos();
        let (sp, cp) = (pitch * 0.5).sin_cos();
        Ok(FirstPersonLookReceipt {
            before: state,
            after: FirstPersonLookState {
                yaw_radians: yaw,
                pitch_radians: pitch,
            },
            orientation: Quat::new(-sp * sy, sp * cy, cp * sy, cp * cy),
            forward,
            right,
            up,
        })
    }
}

fn validate_look_config(config: &FirstPersonLookConfig) -> Result<(), FirstPersonLookError> {
    let finite = [
        config.horizontal_radians_per_unit,
        config.vertical_radians_per_unit,
        config.minimum_pitch_radians,
        config.maximum_pitch_radians,
        config.maximum_delta_radians,
    ]
    .into_iter()
    .all(f32::is_finite);
    if !finite
        || config.horizontal_radians_per_unit < 0.0
        || config.vertical_radians_per_unit < 0.0
        || config.minimum_pitch_radians >= config.maximum_pitch_radians
        || config.minimum_pitch_radians < -std::f32::consts::FRAC_PI_2
        || config.maximum_pitch_radians > std::f32::consts::FRAC_PI_2
        || config.maximum_delta_radians <= 0.0
    {
        Err(FirstPersonLookError::InvalidConfig)
    } else {
        Ok(())
    }
}

fn wrap_radians(value: f32) -> f32 {
    (value + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn positive(field: &'static str, value: f32) -> Result<(), CharacterConfigError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(CharacterConfigError::new(field))
    }
}

fn nonnegative(field: &'static str, value: f32) -> Result<(), CharacterConfigError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(CharacterConfigError::new(field))
    }
}

fn range(field: &'static str, value: f32, min: f32, max: f32) -> Result<(), CharacterConfigError> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(CharacterConfigError::new(field))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_config_documents_inherit_defaults() {
        let config: CharacterControllerConfig =
            serde_json::from_str(r#"{"ground":{"forwardSpeed":7.5}}"#).unwrap();
        assert_eq!(config.ground.forward_speed, 7.5);
        assert_eq!(
            config.ground.strafe_speed,
            CharacterGroundConfig::default().strafe_speed
        );
        assert_eq!(config.shape, CharacterShapeConfig::default());
        assert_eq!(config.solver, CharacterSolverConfig::default());
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn validation_names_invalid_field() {
        let mut config = CharacterControllerConfig::default();
        config.shape.radius = f32::NAN;
        assert_eq!(config.validate().unwrap_err().field, "shape.radius");
    }

    #[test]
    fn look_uses_engine_heading_convention() {
        let receipt = FirstPersonLookService
            .integrate(
                &FirstPersonLookConfig::default(),
                FirstPersonLookState::default(),
                FirstPersonLookCommand {
                    delta: Vec2::new(std::f32::consts::FRAC_PI_2, 0.0),
                },
            )
            .unwrap();
        assert!((receipt.forward.x - 1.0).abs() < 1.0e-5);
        assert!(receipt.forward.y.abs() < 1.0e-5);
        assert!(receipt.forward.z.abs() < 1.0e-5);
    }

    #[test]
    fn look_clamps_pitch_and_bounds_delta() {
        let config = FirstPersonLookConfig::default();
        let receipt = FirstPersonLookService
            .integrate(
                &config,
                FirstPersonLookState::default(),
                FirstPersonLookCommand {
                    delta: Vec2::new(0.0, 2.0),
                },
            )
            .unwrap();
        assert_eq!(receipt.after.pitch_radians, config.maximum_pitch_radians);
        assert_eq!(
            FirstPersonLookService.integrate(
                &config,
                FirstPersonLookState::default(),
                FirstPersonLookCommand {
                    delta: Vec2::new(4.0, 0.0)
                },
            ),
            Err(FirstPersonLookError::DeltaLimitExceeded)
        );
    }
}
