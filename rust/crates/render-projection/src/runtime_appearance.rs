//! Product-selected runtime appearances projected through Engine-owned retained state.
//!
//! A product selects stable object identities and admitted appearance identities. This
//! module resolves that compact product decision into the retained renderer-neutral
//! scene; it deliberately does not expose handles, resources, or frame operations to
//! the product runtime.

use std::collections::{BTreeMap, BTreeSet};

use render_model::{
    LightDescriptor, RenderFrameDiff, RenderLayer, RenderMetadata, Transform, JSON_SAFE_U64_MAX,
};
use serde::Deserialize;

use crate::{
    Appearance, AppearanceLight, AppearanceNode, AppearanceResources, AppearanceScene,
    ProjectionAvailability, ProjectionMode, SceneAppearanceProjector, SceneProjectionError,
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
    pub parent_object_id: Option<u64>,
    pub appearance: String,
    pub transform: Transform,
    pub visible: bool,
    pub layer: RenderLayer,
}

/// One complete product fact for a retained runtime light. Light identities
/// share the Engine-owned scene projector with appearance objects, but remain
/// a distinct logical key so products never observe renderer handles.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeLightFact {
    pub light_id: u64,
    pub parent_object_id: Option<u64>,
    pub light: LightDescriptor,
}

/// Engine-owned resolver and retained projection state for one product runtime.
#[derive(Debug, Clone)]
pub struct RuntimeAppearanceProjector {
    catalog: RuntimeAppearanceCatalog,
    projector: SceneAppearanceProjector,
    appearance_facts: Vec<RuntimeAppearanceFact>,
    light_facts: Vec<RuntimeLightFact>,
}

impl RuntimeAppearanceProjector {
    pub fn new(catalog: RuntimeAppearanceCatalog) -> Self {
        Self {
            catalog,
            projector: SceneAppearanceProjector::new(),
            appearance_facts: Vec::new(),
            light_facts: Vec::new(),
        }
    }

    /// Starts a detached renderer projection while preserving the current
    /// logical appearance and light facts. The next complete product snapshot
    /// therefore emits a full create baseline for a fresh browser without
    /// disturbing the retained projector used by active browsers.
    pub fn reset_renderer_projection(&mut self) {
        self.projector = SceneAppearanceProjector::new();
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

    /// Releases an unused mesh definition and publishes its removal through the
    /// same retained resource diff as ordinary snapshots. A never-published
    /// resource needs no renderer release. Failure preserves the current state.
    pub fn release_static_mesh(
        &mut self,
        asset: &str,
    ) -> Result<RuntimeAppearanceProjection, RuntimeAppearanceProjectionError> {
        let mut next = self.clone();
        next.catalog
            .resources
            .static_meshes
            .retain(|mesh| mesh.asset != asset);
        let projection =
            next.project_scene(next.appearance_facts.clone(), next.light_facts.clone())?;
        *self = next;
        Ok(projection)
    }

    /// Projects one complete snapshot. Omitted object identities are destroyed by the
    /// retained projector; the Engine-owned catalog remains available for later facts.
    pub fn project(
        &mut self,
        facts: &[RuntimeAppearanceFact],
    ) -> Result<RuntimeAppearanceProjection, RuntimeAppearanceProjectionError> {
        self.project_appearance_snapshot(facts)
    }

    /// Projects one complete appearance snapshot while retaining the current
    /// light facts. Omitted object identities are destroyed by the retained
    /// projector; the Engine-owned catalog remains available for later facts.
    pub fn project_appearance_snapshot(
        &mut self,
        facts: &[RuntimeAppearanceFact],
    ) -> Result<RuntimeAppearanceProjection, RuntimeAppearanceProjectionError> {
        self.project_scene(facts.to_vec(), self.light_facts.clone())
    }

    /// Projects the current logical light set while retaining the complete
    /// appearance snapshot. This intentionally uses the same scene allocator
    /// as appearances, allowing a light and an object to use the same logical
    /// numeric identity without renderer-handle collisions.
    pub fn project_lights(
        &mut self,
        facts: &[RuntimeLightFact],
    ) -> Result<RuntimeAppearanceProjection, RuntimeAppearanceProjectionError> {
        self.project_scene(self.appearance_facts.clone(), facts.to_vec())
    }

    fn project_scene(
        &mut self,
        appearance_facts: Vec<RuntimeAppearanceFact>,
        light_facts: Vec<RuntimeLightFact>,
    ) -> Result<RuntimeAppearanceProjection, RuntimeAppearanceProjectionError> {
        let mut object_ids = BTreeSet::new();
        let mut nodes = Vec::with_capacity(appearance_facts.len());
        for fact in &appearance_facts {
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
                parent: fact.parent_object_id,
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
        let mut light_ids = BTreeSet::new();
        let mut lights = Vec::with_capacity(light_facts.len());
        for fact in &light_facts {
            if !light_ids.insert(fact.light_id) {
                return Err(RuntimeAppearanceProjectionError::DuplicateLight {
                    light_id: fact.light_id,
                });
            }
            if fact.light_id > JSON_SAFE_U64_MAX {
                return Err(RuntimeAppearanceProjectionError::UnsafeLightId {
                    light_id: fact.light_id,
                });
            }
            lights.push(AppearanceLight {
                id: fact.light_id,
                parent: fact.parent_object_id,
                availability: ProjectionAvailability::RuntimeOnly,
                light: fact.light.clone(),
            });
        }
        let scene = AppearanceScene {
            resources: self.catalog.resources.clone(),
            nodes,
            lights,
        };
        let result = self
            .projector
            .project(&scene, ProjectionMode::Runtime)
            .map_err(RuntimeAppearanceProjectionError::Scene)?;
        self.appearance_facts = appearance_facts;
        self.light_facts = light_facts;
        Ok(RuntimeAppearanceProjection {
            frame: result.frame,
            retained_objects: result.readout.retained_nodes,
            retained_lights: result.readout.retained_lights,
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
    pub retained_lights: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeAppearanceProjectionError {
    DuplicateObject { object_id: u64 },
    UnsafeObjectId { object_id: u64 },
    DuplicateLight { light_id: u64 },
    UnsafeLightId { light_id: u64 },
    UnknownAppearance { appearance: String },
    Scene(SceneProjectionError),
}

#[cfg(test)]
mod tests {
    use render_model::{Geometry, LightShadowIntent, Material, RenderDiff};

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
            parent_object_id: None,
            appearance: "appearance/trial".to_owned(),
            transform: Transform::IDENTITY,
            visible: true,
            layer: RenderLayer::Scene,
        }
    }

    fn ambient_light(id: u64, parent_object_id: Option<u64>) -> RuntimeLightFact {
        RuntimeLightFact {
            light_id: id,
            parent_object_id,
            light: LightDescriptor::Ambient {
                color: [0.2, 0.3, 0.4],
                intensity: 0.5,
                enabled: true,
                shadow_intent: LightShadowIntent::Requested,
            },
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
    fn appearance_parents_create_in_order_and_recreate_descendants() {
        let mut projector = RuntimeAppearanceProjector::new(catalog());
        let parent = fact(7);
        let child = RuntimeAppearanceFact {
            object_id: 8,
            parent_object_id: Some(parent.object_id),
            ..fact(8)
        };
        let initial = projector.project(&[child.clone(), parent.clone()]).unwrap();
        assert!(matches!(
            initial.frame.ops.as_slice(),
            [
                RenderDiff::Create { .. },
                RenderDiff::Create {
                    parent: Some(_),
                    ..
                },
            ]
        ));

        let reparented = RuntimeAppearanceFact {
            parent_object_id: None,
            ..child
        };
        let moved = projector.project(&[parent, reparented]).unwrap();
        assert!(matches!(
            moved.frame.ops.as_slice(),
            [
                RenderDiff::Destroy { .. },
                RenderDiff::Create { parent: None, .. },
            ]
        ));
    }

    #[test]
    fn invalid_appearance_parent_does_not_commit_prior_snapshot() {
        let mut projector = RuntimeAppearanceProjector::new(catalog());
        projector.project(&[fact(7)]).unwrap();
        let invalid = RuntimeAppearanceFact {
            parent_object_id: Some(99),
            ..fact(7)
        };
        assert!(matches!(
            projector.project(&[invalid]),
            Err(RuntimeAppearanceProjectionError::Scene(
                SceneProjectionError::MissingParent { .. }
            ))
        ));
        assert!(projector.project(&[fact(7)]).unwrap().frame.is_empty());
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

    #[test]
    fn appearance_snapshots_and_light_mutations_share_one_retained_scene() {
        let mut projector = RuntimeAppearanceProjector::new(catalog());
        let node = projector.project(&[fact(7)]).unwrap();
        let node_handle = projector.object_handle(7).unwrap();
        assert!(matches!(
            node.frame.ops.as_slice(),
            [RenderDiff::Create { .. }]
        ));

        let light = projector
            .project_lights(&[ambient_light(7, Some(7))])
            .unwrap();
        assert_eq!(light.retained_objects, 1);
        assert_eq!(light.retained_lights, 1);
        assert!(matches!(
            light.frame.ops.as_slice(),
            [RenderDiff::CreateLight { parent: Some(parent), .. }] if *parent == node_handle
        ));

        let mut moved = fact(7);
        moved.transform.translation = [3.0, 0.0, 0.0];
        let appearance_update = projector.project(&[moved]).unwrap();
        assert!(matches!(
            appearance_update.frame.ops.as_slice(),
            [RenderDiff::Update { handle, .. }] if *handle == node_handle
        ));
        assert_eq!(appearance_update.retained_lights, 1);

        let mut updated_light = ambient_light(7, Some(7));
        if let LightDescriptor::Ambient { intensity, .. } = &mut updated_light.light {
            *intensity = 1.0;
        }
        let light_update = projector.project_lights(&[updated_light]).unwrap();
        assert!(matches!(
            light_update.frame.ops.as_slice(),
            [RenderDiff::UpdateLight { .. }]
        ));
        assert_eq!(light_update.retained_objects, 1);
    }
}
