use core_assets::AssetReference;
use core_ids::{EntityId, ProcessId, SubjectId};
use core_math::Vec3;

use crate::value::{EntityTransform, Quat};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformCapability {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl TransformCapability {
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundsCapability {
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionCapability {
    pub enabled: bool,
    pub static_collider: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderableCapability {
    pub visible: bool,
    pub asset: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KinematicCapability {
    pub half_extents: Vec3,
    pub velocity: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerCapability {
    Process(ProcessId),
    Subject(SubjectId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetBindingCapability {
    pub asset: AssetReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainmentCapability {
    pub container: EntityId,
}
