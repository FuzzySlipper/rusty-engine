use core_assets::AssetReference;
use core_ids::{ProcessId, SubjectId};
use core_math::Vec3;

use crate::component::EntityComponent;
use crate::value::{EntityTransform, Quat};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformComponent {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl TransformComponent {
    pub const fn from_transform(transform: EntityTransform) -> Self {
        Self {
            translation: transform.translation,
            rotation: transform.rotation,
            scale: transform.scale,
        }
    }

    pub const fn transform(self) -> EntityTransform {
        EntityTransform {
            translation: self.translation,
            rotation: self.rotation,
            scale: self.scale,
        }
    }
}

impl EntityComponent for TransformComponent {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundsComponent {
    pub min: Vec3,
    pub max: Vec3,
}

impl EntityComponent for BoundsComponent {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionComponent {
    pub enabled: bool,
    pub static_collider: bool,
}

impl EntityComponent for CollisionComponent {}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderableComponent {
    pub visible: bool,
    pub asset: String,
    /// Presentation-only transform composed after the entity world transform.
    ///
    /// Spatial, collision, navigation, and gameplay owners continue to observe
    /// the entity transform without this local visual correction.
    pub local_transform: EntityTransform,
}

impl EntityComponent for RenderableComponent {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KinematicComponent {
    pub half_extents: Vec3,
    pub velocity: Vec3,
}

impl EntityComponent for KinematicComponent {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterStance {
    Standing,
    Crouched,
}

/// Canonical inert continuation facts for one kinematic character.
///
/// Collision contacts and solver caches remain derived readouts. The stable
/// support anchor and timing facts live here so downstream callers cannot lose
/// them between direct service calls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterMotionComponent {
    pub controlled_velocity: Vec3,
    pub external_velocity: Vec3,
    pub stance: CharacterStance,
    pub grounded: bool,
    pub jump_buffer_remaining: f32,
    pub coyote_remaining: f32,
    pub landing_lockout_remaining: f32,
    pub support_entity: Option<core_ids::EntityId>,
    pub support_local_anchor: Vec3,
    pub support_previous_translation: Vec3,
    pub support_previous_rotation: Quat,
    pub support_point_velocity: Vec3,
    pub fall_origin_y: f32,
    pub peak_y: f32,
    pub last_command_sequence: u64,
    pub collision_world_hash: u64,
}

impl CharacterMotionComponent {
    pub const fn at_rest(height: f32) -> Self {
        Self {
            controlled_velocity: Vec3::ZERO,
            external_velocity: Vec3::ZERO,
            stance: CharacterStance::Standing,
            grounded: false,
            jump_buffer_remaining: 0.0,
            coyote_remaining: 0.0,
            landing_lockout_remaining: 0.0,
            support_entity: None,
            support_local_anchor: Vec3::ZERO,
            support_previous_translation: Vec3::ZERO,
            support_previous_rotation: Quat::IDENTITY,
            support_point_velocity: Vec3::ZERO,
            fall_origin_y: height,
            peak_y: height,
            last_command_sequence: 0,
            collision_world_hash: 0,
        }
    }
}

impl EntityComponent for CharacterMotionComponent {}

/// The only non-kinematic body mode admitted by the first rigid-body contract.
///
/// Static collision remains owned by canonical voxel/static-mesh projections and
/// character/controller motion remains on [`KinematicComponent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigidBodyMode {
    Dynamic,
}

/// Bounded primitive geometry admitted for a dynamic rigid body.
///
/// Triangle meshes are deliberately absent: they are supported only as static
/// environment collision and are rejected by the rigid-body admission service.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RigidBodyShape {
    Sphere { radius: f32 },
    Cuboid { half_extents: Vec3 },
    CapsuleY { half_height: f32, radius: f32 },
}

/// Solver-owned inertia policy for one admitted dynamic body.
///
/// `mass` on [`RigidBodyComponent`] is always the authoritative total mass.
/// The explicit branch supplies the remaining mass properties in the body's
/// local space; it does not add another mass value or a second collider mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RigidBodyInertiaPolicy {
    DeriveFromShapeAndMass,
    Explicit {
        center_of_mass: Vec3,
        principal_inertia: Vec3,
        principal_inertia_local_frame: Quat,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBodyComponent {
    pub mode: RigidBodyMode,
    pub shape: RigidBodyShape,
    pub mass: f32,
    pub inertia: RigidBodyInertiaPolicy,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    /// Axes whose world-space translation is constrained by the canonical
    /// rigid-body solver. `true` means the corresponding X/Y/Z axis is locked.
    pub locked_translation_axes: [bool; 3],
    /// Axes whose world-space rotation is constrained by the canonical
    /// rigid-body solver. `true` means the corresponding X/Y/Z axis is locked.
    pub locked_rotation_axes: [bool; 3],
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub gravity_scale: f32,
    pub friction: f32,
    pub restitution: f32,
    pub collision_groups: u32,
    pub collision_mask: u32,
    pub enabled: bool,
    pub sleeping: bool,
    pub continuous_collision: bool,
}

impl RigidBodyComponent {
    pub const fn dynamic(shape: RigidBodyShape, mass: f32) -> Self {
        Self {
            mode: RigidBodyMode::Dynamic,
            shape,
            mass,
            inertia: RigidBodyInertiaPolicy::DeriveFromShapeAndMass,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            locked_translation_axes: [false; 3],
            locked_rotation_axes: [false; 3],
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

    /// Select authored local mass properties while preserving this body's
    /// authoritative total mass and every other dynamic-body default.
    pub const fn with_explicit_inertia(
        mut self,
        center_of_mass: Vec3,
        principal_inertia: Vec3,
        principal_inertia_local_frame: Quat,
    ) -> Self {
        self.inertia = RigidBodyInertiaPolicy::Explicit {
            center_of_mass,
            principal_inertia,
            principal_inertia_local_frame,
        };
        self
    }
}

impl EntityComponent for RigidBodyComponent {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerComponent {
    Process(ProcessId),
    Subject(SubjectId),
}

impl EntityComponent for ControllerComponent {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetBindingComponent {
    pub asset: AssetReference,
}

impl EntityComponent for AssetBindingComponent {}
