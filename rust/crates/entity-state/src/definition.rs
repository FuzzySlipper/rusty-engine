use std::collections::BTreeSet;

use core_assets::AssetReference;
use core_ids::{EntityId, TagId};
use core_math::Vec3;

use crate::capability::{
    AssetBindingCapability, BoundsCapability, CollisionCapability, ControllerCapability,
    KinematicCapability, RenderableCapability, TransformCapability,
};
use crate::model::EntitySource;
use crate::value::EntityTransform;

pub const MAX_ABS_TRANSLATION: f32 = 1_000_000.0;
pub const MAX_ABS_VELOCITY: f32 = 10_000.0;

#[derive(Debug, Clone, PartialEq)]
pub struct EntityDefinition {
    pub id: EntityId,
    pub name: String,
    pub source: EntitySource,
    pub labels: Vec<TagId>,
    pub transform: Option<TransformCapability>,
    pub bounds: Option<BoundsCapability>,
    pub collision: Option<CollisionCapability>,
    pub renderable: Option<RenderableCapability>,
    pub kinematic: Option<KinematicCapability>,
    pub controller: Option<ControllerCapability>,
    pub asset_binding: Option<AssetBindingCapability>,
    pub transform_parent: Option<EntityId>,
    pub contained_in: Option<EntityId>,
    pub derived_from: Option<EntityId>,
}

impl EntityDefinition {
    pub fn new(id: EntityId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            source: EntitySource::default(),
            labels: Vec::new(),
            transform: None,
            bounds: None,
            collision: None,
            renderable: None,
            kinematic: None,
            controller: None,
            asset_binding: None,
            transform_parent: None,
            contained_in: None,
            derived_from: None,
        }
    }

    pub fn with_source(mut self, source: EntitySource) -> Self {
        self.source = source;
        self
    }

    pub fn with_labels(mut self, labels: impl IntoIterator<Item = TagId>) -> Self {
        self.labels = labels.into_iter().collect();
        self
    }

    pub fn with_transform(mut self, translation: Vec3) -> Self {
        self.transform = Some(TransformCapability::from_transform(EntityTransform::at(
            translation,
        )));
        self
    }

    pub fn with_full_transform(mut self, transform: EntityTransform) -> Self {
        self.transform = Some(TransformCapability::from_transform(transform));
        self
    }

    pub fn with_bounds(mut self, min: Vec3, max: Vec3) -> Self {
        self.bounds = Some(BoundsCapability { min, max });
        self
    }

    pub fn with_collision(mut self, enabled: bool, static_collider: bool) -> Self {
        self.collision = Some(CollisionCapability {
            enabled,
            static_collider,
        });
        self
    }

    pub fn with_renderable(mut self, asset: impl Into<String>, visible: bool) -> Self {
        self.renderable = Some(RenderableCapability {
            visible,
            asset: asset.into(),
        });
        self
    }

    pub fn with_kinematic(mut self, half_extents: Vec3, velocity: Vec3) -> Self {
        self.kinematic = Some(KinematicCapability {
            half_extents,
            velocity,
        });
        self
    }

    pub fn with_controller(mut self, controller: ControllerCapability) -> Self {
        self.controller = Some(controller);
        self
    }

    pub fn with_asset_binding(mut self, asset: AssetReference) -> Self {
        self.asset_binding = Some(AssetBindingCapability { asset });
        self
    }

    pub fn with_transform_parent(mut self, parent: EntityId) -> Self {
        self.transform_parent = Some(parent);
        self
    }

    pub fn with_containment(mut self, container: EntityId) -> Self {
        self.contained_in = Some(container);
        self
    }

    pub fn with_derivation(mut self, origin: EntityId) -> Self {
        self.derived_from = Some(origin);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityDefinitionError {
    DuplicateEntity { entity: EntityId },
    EmptyName { entity: EntityId },
    DuplicateLabel { entity: EntityId, label: TagId },
    InvalidSource { entity: EntityId },
    InvalidTransform { entity: EntityId },
    InvalidBounds { entity: EntityId },
    EmptyAsset { entity: EntityId },
    KinematicMissingTransform { entity: EntityId },
    InvalidKinematicHalfExtents { entity: EntityId },
    InvalidKinematicVelocity { entity: EntityId },
    InvalidRelationship { entity: EntityId, reason: String },
}

impl std::fmt::Display for EntityDefinitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EntityDefinitionError {}

pub(crate) fn validate_definition(
    definition: &EntityDefinition,
) -> Result<(), EntityDefinitionError> {
    if definition.name.trim().is_empty() {
        return Err(EntityDefinitionError::EmptyName {
            entity: definition.id,
        });
    }
    let mut labels = BTreeSet::new();
    for &label in &definition.labels {
        if !labels.insert(label) {
            return Err(EntityDefinitionError::DuplicateLabel {
                entity: definition.id,
                label,
            });
        }
    }
    if let EntitySource::PrefabInstance {
        role: Some(role), ..
    } = &definition.source
    {
        if role.trim().is_empty() {
            return Err(EntityDefinitionError::InvalidSource {
                entity: definition.id,
            });
        }
    }
    if definition.transform.is_some_and(|value| {
        !transform_is_valid(EntityTransform {
            translation: value.translation,
            rotation: value.rotation,
            scale: value.scale,
        })
    }) {
        return Err(EntityDefinitionError::InvalidTransform {
            entity: definition.id,
        });
    }
    if definition
        .bounds
        .is_some_and(|value| !bounds_are_valid(value))
    {
        return Err(EntityDefinitionError::InvalidBounds {
            entity: definition.id,
        });
    }
    if definition
        .renderable
        .as_ref()
        .is_some_and(|value| value.asset.trim().is_empty())
    {
        return Err(EntityDefinitionError::EmptyAsset {
            entity: definition.id,
        });
    }
    if let Some(kinematic) = definition.kinematic {
        if definition.transform.is_none() {
            return Err(EntityDefinitionError::KinematicMissingTransform {
                entity: definition.id,
            });
        }
        if !half_extents_are_valid(kinematic.half_extents) {
            return Err(EntityDefinitionError::InvalidKinematicHalfExtents {
                entity: definition.id,
            });
        }
        if !velocity_is_valid(kinematic.velocity) {
            return Err(EntityDefinitionError::InvalidKinematicVelocity {
                entity: definition.id,
            });
        }
    }
    Ok(())
}

pub(crate) fn translation_is_valid(value: Vec3) -> bool {
    vector_is_finite(value)
        && value.x.abs() <= MAX_ABS_TRANSLATION
        && value.y.abs() <= MAX_ABS_TRANSLATION
        && value.z.abs() <= MAX_ABS_TRANSLATION
}

pub(crate) fn velocity_is_valid(value: Vec3) -> bool {
    vector_is_finite(value)
        && value.x.abs() <= MAX_ABS_VELOCITY
        && value.y.abs() <= MAX_ABS_VELOCITY
        && value.z.abs() <= MAX_ABS_VELOCITY
}

pub(crate) fn transform_is_valid(value: EntityTransform) -> bool {
    translation_is_valid(value.translation)
        && vector_is_finite(value.scale)
        && value.scale.x > 0.0
        && value.scale.y > 0.0
        && value.scale.z > 0.0
        && value.rotation.x.is_finite()
        && value.rotation.y.is_finite()
        && value.rotation.z.is_finite()
        && value.rotation.w.is_finite()
        && (value.rotation.norm_squared() - 1.0).abs() <= 0.001
}

pub(crate) fn bounds_are_valid(value: BoundsCapability) -> bool {
    translation_is_valid(value.min)
        && translation_is_valid(value.max)
        && value.min.x <= value.max.x
        && value.min.y <= value.max.y
        && value.min.z <= value.max.z
}

pub(crate) fn half_extents_are_valid(value: Vec3) -> bool {
    translation_is_valid(value) && value.x > 0.0 && value.y > 0.0 && value.z > 0.0
}

fn vector_is_finite(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}
