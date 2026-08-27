//! Product-selected runtime appearances projected through Engine-owned retained state.
//!
//! A product selects stable object identities and admitted appearance identities. This
//! module resolves that compact product decision into the retained renderer-neutral
//! scene; it deliberately does not expose handles, resources, or frame operations to
//! the product runtime.

use std::collections::{BTreeMap, BTreeSet};

use render_model::{RenderFrameDiff, RenderLayer, RenderMetadata, Transform, JSON_SAFE_U64_MAX};
use serde::Deserialize;

use crate::{
    Appearance, AppearanceNode, AppearanceResources, AppearanceScene, ProjectionAvailability,
    ProjectionMode, SceneAppearanceProjector, SceneProjectionError,
};

/// Admitted Engine content available to a trusted product runtime.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeAppearanceCatalog {
    #[serde(default)]
    pub resources: AppearanceResources,
    pub appearances: BTreeMap<String, Appearance>,
}

impl RuntimeAppearanceCatalog {
    /// Adds or replaces one product-authored appearance and returns the prior value.
    pub fn insert_appearance(
        &mut self,
        identity: String,
        appearance: Appearance,
    ) -> Option<Appearance> {
        self.appearances.insert(identity, appearance)
    }

    /// Mutable access for trusted composition roots that directly admit renderer resources.
    pub fn resources_mut(&mut self) -> &mut AppearanceResources {
        &mut self.resources
    }
}

/// One complete product fact for the current retained visual snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeAppearanceFact {
    pub object_id: u64,
    pub appearance: String,
    pub transform: Transform,
    pub visible: bool,
    pub layer: RenderLayer,
}

/// Engine-owned resolver and retained projection state for one product runtime.
#[derive(Debug, Clone)]
pub struct RuntimeAppearanceProjector {
    catalog: RuntimeAppearanceCatalog,
    projector: SceneAppearanceProjector,
}

impl RuntimeAppearanceProjector {
    pub fn new(catalog: RuntimeAppearanceCatalog) -> Self {
        Self {
            catalog,
            projector: SceneAppearanceProjector::new(),
        }
    }

    /// Adds or replaces one product-authored appearance without resetting retained objects.
    pub fn insert_appearance(
        &mut self,
        identity: String,
        appearance: Appearance,
    ) -> Option<Appearance> {
        self.catalog.insert_appearance(identity, appearance)
    }

    /// Removes one admitted runtime appearance. Existing projected objects are
    /// still removed through the next complete product snapshot; callers may
    /// not retain or operate renderer handles directly.
    pub fn remove_appearance(&mut self, identity: &str) -> Option<Appearance> {
        self.catalog.appearances.remove(identity)
    }

    /// Mutates an already admitted appearance definition. The next complete
    /// snapshot remains the only path that changes retained renderer objects.
    pub fn appearance_mut(&mut self, identity: &str) -> Option<&mut Appearance> {
        self.catalog.appearances.get_mut(identity)
    }

    /// Mutable resource access for direct trusted product admission.
    pub fn resources_mut(&mut self) -> &mut AppearanceResources {
        self.catalog.resources_mut()
    }

    /// Projects one complete snapshot. Omitted object identities are destroyed by the
    /// retained projector; the Engine-owned catalog remains available for later facts.
    pub fn project(
        &mut self,
        facts: &[RuntimeAppearanceFact],
    ) -> Result<RuntimeAppearanceProjection, RuntimeAppearanceProjectionError> {
        let mut object_ids = BTreeSet::new();
        let mut nodes = Vec::with_capacity(facts.len());
        for fact in facts {
            if !object_ids.insert(fact.object_id) {
                return Err(RuntimeAppearanceProjectionError::DuplicateObject {
                    object_id: fact.object_id,
                });
            }
            if fact.object_id > JSON_SAFE_U64_MAX {
                return Err(RuntimeAppearanceProjectionError::UnsafeObjectId {
                    object_id: fact.object_id,
                });
            }
            let appearance = self
                .catalog
                .appearances
                .get(&fact.appearance)
                .cloned()
                .ok_or_else(|| RuntimeAppearanceProjectionError::UnknownAppearance {
                    appearance: fact.appearance.clone(),
                })?;
            nodes.push(AppearanceNode {
                id: fact.object_id,
                parent: None,
                transform: fact.transform,
                visible: fact.visible,
                layer: fact.layer,
                metadata: RenderMetadata {
                    source_entity: Some(fact.object_id),
                    source_scene_node: None,
                    tags: Vec::new(),
                    label: Some(fact.appearance.clone()),
                },
                availability: ProjectionAvailability::RuntimeOnly,
                appearance,
            });
        }
        let scene = AppearanceScene {
            resources: self.catalog.resources.clone(),
            nodes,
            lights: Vec::new(),
        };
        let result = self
            .projector
            .project(&scene, ProjectionMode::Runtime)
            .map_err(RuntimeAppearanceProjectionError::Scene)?;
        Ok(RuntimeAppearanceProjection {
            frame: result.frame,
            retained_objects: result.readout.retained_nodes,
        })
    }

    pub fn object_handle(&self, object_id: u64) -> Option<render_model::RenderHandle> {
        self.projector.node_handle(object_id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeAppearanceProjection {
    pub frame: RenderFrameDiff,
    pub retained_objects: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeAppearanceProjectionError {
    DuplicateObject { object_id: u64 },
    UnsafeObjectId { object_id: u64 },
    UnknownAppearance { appearance: String },
    Scene(SceneProjectionError),
}

#[cfg(test)]
mod tests {
    use render_model::{Geometry, Material, RenderDiff};

    use super::*;

    fn catalog() -> RuntimeAppearanceCatalog {
        RuntimeAppearanceCatalog {
            resources: AppearanceResources::default(),
            appearances: BTreeMap::from([(
                "appearance/trial".to_owned(),
                Appearance::Primitive {
                    geometry: Geometry::Cube,
                    material: Material::DEFAULT,
                },
            )]),
        }
    }

    fn fact(id: u64) -> RuntimeAppearanceFact {
        RuntimeAppearanceFact {
            object_id: id,
            appearance: "appearance/trial".to_owned(),
            transform: Transform::IDENTITY,
            visible: true,
            layer: RenderLayer::Scene,
        }
    }

    #[test]
    fn stable_product_identity_updates_and_snapshot_absence_destroys() {
        let mut projector = RuntimeAppearanceProjector::new(catalog());
        let created = projector.project(&[fact(7)]).unwrap();
        assert!(matches!(
            created.frame.ops.as_slice(),
            [RenderDiff::Create { .. }]
        ));
        let handle = projector.object_handle(7).unwrap();

        let mut moved = fact(7);
        moved.transform.translation = [3.0, 0.0, 0.0];
        let updated = projector.project(&[moved]).unwrap();
        assert!(
            matches!(updated.frame.ops.as_slice(), [RenderDiff::Update { handle: updated_handle, .. }] if *updated_handle == handle)
        );

        let removed = projector.project(&[]).unwrap();
        assert!(
            matches!(removed.frame.ops.as_slice(), [RenderDiff::Destroy { handle: destroyed }] if *destroyed == handle)
        );
    }

    #[test]
    fn unknown_appearance_does_not_commit_prior_snapshot() {
        let mut projector = RuntimeAppearanceProjector::new(catalog());
        projector.project(&[fact(7)]).unwrap();
        let handle = projector.object_handle(7).unwrap();
        let invalid = RuntimeAppearanceFact {
            appearance: "appearance/missing".to_owned(),
            ..fact(7)
        };
        assert!(matches!(
            projector.project(&[invalid]),
            Err(RuntimeAppearanceProjectionError::UnknownAppearance { .. })
        ));
        assert_eq!(projector.object_handle(7), Some(handle));
        assert!(projector.project(&[fact(7)]).unwrap().frame.is_empty());
    }
}
