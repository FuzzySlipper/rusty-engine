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

/// Solver-owned inertia derived from one admitted shape and the authored mass.
///
/// Custom tensors are intentionally not part of schema 1. This keeps invalid or
/// non-positive inertia out of durable state while still making the policy
/// explicit and forward-versionable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigidBodyInertiaPolicy {
    DeriveFromShapeAndMass,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBodyComponent {
    pub mode: RigidBodyMode,
    pub shape: RigidBodyShape,
    pub mass: f32,
    pub inertia: RigidBodyInertiaPolicy,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
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
