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
