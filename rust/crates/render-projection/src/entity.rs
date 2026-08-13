use std::collections::BTreeMap;

use core_ids::EntityId;
use entity_state::{EntityState, EntityTransform};
use render_model::{
    AnimatedMeshInstanceDescriptor, BillboardMode, RenderAssetError, RenderAssetKind, RenderDiff,
    RenderFrameDiff, RenderFrameError, RenderHandle, RenderMetadata, ResolvedRenderAsset,
    SpriteAttachment, SpriteDepthPolicy, SpriteInstanceDescriptor, SpriteShading, SpriteSizeMode,
    StaticMeshInstanceDescriptor, Transform,
};

use crate::{HandleAllocationError, RenderHandleNamespace, StableHandleRegistry};

pub trait RenderAssetLookup {
    fn get_render_asset(&self, id: &str) -> Option<&ResolvedRenderAsset>;
}

impl RenderAssetLookup for BTreeMap<String, ResolvedRenderAsset> {
    fn get_render_asset(&self, id: &str) -> Option<&ResolvedRenderAsset> {
        self.get(id)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ProjectedEntity {
    asset: String,
    kind: RenderAssetKind,
    transform: Transform,
    visible: bool,
    metadata: RenderMetadata,
}

#[derive(Debug, Clone)]
pub struct EntityRenderProjector {
    registry: StableHandleRegistry<EntityId>,
    last: BTreeMap<EntityId, ProjectedEntity>,
}

impl Default for EntityRenderProjector {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityRenderProjector {
    pub fn new() -> Self {
        Self {
            registry: StableHandleRegistry::new(RenderHandleNamespace::ENTITY),
            last: BTreeMap::new(),
        }
    }

    /// Projects the public read-only entity view. Missing resources become
    /// explicit diagnostics; malformed resource records fail the complete call.
    pub fn project(
        &mut self,
        state: &EntityState,
        assets: &impl RenderAssetLookup,
    ) -> Result<EntityProjectionResult, EntityProjectionError> {
        let mut diagnostics = Vec::new();
        let mut current = BTreeMap::new();
        for node in state.projection() {
            let Some(asset) = assets.get_render_asset(&node.asset) else {
                diagnostics.push(EntityProjectionDiagnostic::MissingAsset {
                    entity: node.entity,
                    asset: node.asset,
                });
                continue;
            };
            asset
                .validate()
                .map_err(|source| EntityProjectionError::InvalidAsset {
                    entity: node.entity,
                    source,
                })?;
            if asset.id != node.asset {
                return Err(EntityProjectionError::MismatchedLookup {
                    requested: node.asset,
                    returned: asset.id.clone(),
                });
            }
            if !matches!(
                asset.kind,
                RenderAssetKind::StaticMesh
                    | RenderAssetKind::AnimatedMesh
                    | RenderAssetKind::Sprite
            ) {
                diagnostics.push(EntityProjectionDiagnostic::UnsupportedAppearance {
                    entity: node.entity,
                    asset: asset.id.clone(),
                    kind: asset.kind,
                });
                continue;
            }
            let transform = node
                .transform
                .unwrap_or(EntityTransform::IDENTITY)
                .compose(node.renderable_local_transform);
            let metadata = RenderMetadata {
                source_entity: Some(node.entity.raw()),
                source_scene_node: None,
                tags: Vec::new(),
                label: Some(node.name),
            };
            current.insert(
                node.entity,
                ProjectedEntity {
                    asset: asset.id.clone(),
                    kind: asset.kind,
                    transform: Transform {
                        translation: [
                            transform.translation.x,
                            transform.translation.y,
                            transform.translation.z,
                        ],
                        rotation: [
                            transform.rotation.x,
                            transform.rotation.y,
                            transform.rotation.z,
                            transform.rotation.w,
                        ],
                        scale: [transform.scale.x, transform.scale.y, transform.scale.z],
                    },
                    visible: node.visible,
                    metadata,
                },
            );
        }

        let mut registry = self.registry.clone();
        let mut operations = Vec::new();
        for (entity, previous) in &self.last {
            let Some(next) = current.get(entity) else {
                let handle = registry
                    .remove(entity)
                    .expect("retained entity has a render handle");
                operations.push(RenderDiff::Destroy { handle });
                continue;
            };
            if previous.kind != next.kind || previous.asset != next.asset {
                let old = registry
                    .remove(entity)
                    .expect("retained entity has a render handle");
                operations.push(RenderDiff::Destroy { handle: old });
                let handle = registry
                    .allocate(*entity)
                    .map_err(EntityProjectionError::Handle)?;
                operations.push(create_operation(handle, next));
                continue;
            }
            if previous != next {
                let handle = registry
                    .handle_of(entity)
                    .expect("retained entity has a render handle");
                operations.push(RenderDiff::Update {
                    handle,
                    transform: (previous.transform != next.transform).then_some(next.transform),
                    material: None,
                    visible: (previous.visible != next.visible).then_some(next.visible),
                    metadata: (previous.metadata != next.metadata).then(|| next.metadata.clone()),
                });
            }
        }
        for (entity, projected) in &current {
            if self.last.contains_key(entity) {
                continue;
            }
            let handle = registry
                .allocate(*entity)
                .map_err(EntityProjectionError::Handle)?;
            operations.push(create_operation(handle, projected));
        }

        let frame =
            RenderFrameDiff::try_from_ops(operations).map_err(EntityProjectionError::Frame)?;
        self.registry = registry;
        self.last = current;
        Ok(EntityProjectionResult {
            frame,
            diagnostics,
            readout: EntityProjectionReadout {
                source_revision: state.revision(),
                retained_entities: self.last.len(),
            },
        })
    }

    pub fn handle_of(&self, entity: EntityId) -> Option<RenderHandle> {
        self.registry.handle_of(&entity)
    }
}

fn create_operation(handle: RenderHandle, entity: &ProjectedEntity) -> RenderDiff {
    match entity.kind {
        RenderAssetKind::StaticMesh => RenderDiff::CreateStaticMeshInstance {
            handle,
            parent: None,
            instance: StaticMeshInstanceDescriptor {
                asset: entity.asset.clone(),
                transform: entity.transform,
                visible: entity.visible,
                material_overrides: Vec::new(),
                metadata: entity.metadata.clone(),
            },
        },
        RenderAssetKind::AnimatedMesh => RenderDiff::CreateAnimatedMeshInstance {
            handle,
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                asset: entity.asset.clone(),
                transform: entity.transform,
                visible: entity.visible,
                material_overrides: Vec::new(),
                playback: None,
                metadata: entity.metadata.clone(),
            },
        },
        RenderAssetKind::Sprite => RenderDiff::CreateSprite {
            handle,
            parent: None,
            sprite: SpriteInstanceDescriptor {
                asset: entity.asset.clone(),
                frame: 0,
                pivot: [0.5, 0.5],
                size: [1.0, 1.0],
                size_mode: SpriteSizeMode::World,
                billboard: BillboardMode::Spherical,
                tint: [1.0; 4],
                render_order: 0,
                depth: SpriteDepthPolicy::Default,
                shading: SpriteShading::Unlit,
                material: Default::default(),
                visible: entity.visible,
                transform: entity.transform,
                attachment: SpriteAttachment {
                    source_entity: entity.metadata.source_entity,
                    source_scene_node: entity.metadata.source_scene_node,
                    attachment_point: None,
                },
                metadata: entity.metadata.clone(),
            },
        },
        _ => unreachable!("unsupported entity appearance filtered before projection"),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityProjectionResult {
    pub frame: RenderFrameDiff,
    pub diagnostics: Vec<EntityProjectionDiagnostic>,
    pub readout: EntityProjectionReadout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityProjectionReadout {
    pub source_revision: u64,
    pub retained_entities: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityProjectionDiagnostic {
    MissingAsset {
        entity: EntityId,
        asset: String,
    },
    UnsupportedAppearance {
        entity: EntityId,
        asset: String,
        kind: RenderAssetKind,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntityProjectionError {
    InvalidAsset {
        entity: EntityId,
        source: RenderAssetError,
    },
    MismatchedLookup {
        requested: String,
        returned: String,
    },
    Handle(HandleAllocationError),
    Frame(RenderFrameError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_math::Vec3;
    use engine_spatial::{
        GlobalPosition, VoxelCollisionScene, WorldOrigin, WorldOriginEntity,
        WorldOriginRebaseRequest, WorldOriginRebaseService, WorldOriginState,
    };
    use entity_state::{EntityDefinition, Quat};

    #[test]
    fn projects_entity_state_with_stable_minimal_updates() {
        let id = EntityId::new(12);
        let mut state = EntityState::from_definitions([EntityDefinition::new(id, "door")
            .with_full_transform(EntityTransform {
                translation: Vec3::new(1.0, 2.0, 3.0),
                rotation: Quat::new(0.0, 0.0, 0.707_106_77, 0.707_106_77),
                scale: Vec3::new(2.0, 3.0, 4.0),
            })
            .with_renderable("mesh/security-door", true)])
        .unwrap();
        let assets = BTreeMap::from([(
            "mesh/security-door".to_string(),
            ResolvedRenderAsset {
                id: "mesh/security-door".to_string(),
                kind: RenderAssetKind::StaticMesh,
                content_hash: Some("d00d".to_string()),
                version: 1,
            },
        )]);
        let mut projector = EntityRenderProjector::new();
        let first = projector.project(&state, &assets).unwrap();
        let handle = projector.handle_of(id).unwrap();
        assert!(matches!(
            &first.frame.ops[0],
            RenderDiff::CreateStaticMeshInstance { handle: actual, instance, .. }
                if *actual == handle
                    && instance.transform.rotation == [0.0, 0.0, 0.707_106_77, 0.707_106_77]
                    && instance.transform.scale == [2.0, 3.0, 4.0]
        ));

        state
            .apply_batch(entity_state::EntityCommandBatch::new([
                entity_state::EntityCommand::SetTranslation {
                    entity: id,
                    translation: Vec3::new(4.0, 2.0, 3.0),
                },
            ]))
            .unwrap();
        let second = projector.project(&state, &assets).unwrap();
        assert!(matches!(
            second.frame.ops[0],
            RenderDiff::Update {
                handle: actual,
                transform: Some(_),
                ..
            } if actual == handle
        ));
    }

    #[test]
    fn world_origin_rebase_updates_entity_transform_without_replacing_handle() {
        let id = EntityId::new(14);
        let mut state = EntityState::from_definitions([EntityDefinition::new(id, "far actor")
            .with_transform(Vec3::new(100_000.25, 2.0, 0.0))
            .with_renderable("mesh/actor", true)])
        .unwrap();
        let assets = BTreeMap::from([(
            "mesh/actor".to_string(),
            ResolvedRenderAsset {
                id: "mesh/actor".to_string(),
                kind: RenderAssetKind::StaticMesh,
                content_hash: Some("actor".to_string()),
                version: 1,
            },
        )]);
        let mut projector = EntityRenderProjector::new();
        projector.project(&state, &assets).unwrap();
        let handle = projector.handle_of(id).unwrap();
        let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
        let mut origin = WorldOriginState::default();
        let request = WorldOriginRebaseRequest {
            expected_origin_revision: 0,
            expected_entity_revision: state.revision(),
            expected_voxel_source_revision: scene.source_revision().raw(),
            expected_static_mesh_revision: scene.static_mesh_collision_revision(),
            target_origin: WorldOrigin::new([100_000, 0, 0]),
            entities: vec![WorldOriginEntity {
                entity: id,
                global_position: GlobalPosition::from_world([100_000.25, 2.0, 0.0]).unwrap(),
            }],
        };
        WorldOriginRebaseService
            .apply(&mut origin, &mut state, &mut scene, request)
            .unwrap();

        let update = projector.project(&state, &assets).unwrap();
        assert_eq!(projector.handle_of(id), Some(handle));
        assert!(matches!(
            update.frame.ops.as_slice(),
            [RenderDiff::Update { handle: actual, transform: Some(transform), .. }]
                if *actual == handle && transform.translation == [0.25, 2.0, 0.0]
        ));
    }

    #[test]
    fn missing_asset_is_typed_and_does_not_create_a_placeholder() {
        let id = EntityId::new(1);
        let state = EntityState::from_definitions([
            EntityDefinition::new(id, "missing").with_renderable("mesh/missing", true)
        ])
        .unwrap();
        let mut projector = EntityRenderProjector::new();
        let result = projector
            .project(&state, &BTreeMap::<String, ResolvedRenderAsset>::new())
            .unwrap();
        assert!(result.frame.is_empty());
        assert!(matches!(
            result.diagnostics[0],
            EntityProjectionDiagnostic::MissingAsset { entity, .. } if entity == id
        ));
    }

    #[test]
    fn renderable_local_transform_changes_only_visual_projection() {
        let id = EntityId::new(13);
        let world = EntityTransform::at(Vec3::new(8.0, 2.5, -3.0));
        let state = EntityState::from_definitions([EntityDefinition::new(id, "grounded-visual")
            .with_full_transform(world)
            .with_collision(true, false)
            .with_renderable("mesh/grounded", true)
            .with_renderable_local_transform(EntityTransform::at(Vec3::new(0.0, -2.5, 0.0)))])
        .unwrap();
        let assets = BTreeMap::from([(
            "mesh/grounded".to_string(),
            ResolvedRenderAsset {
                id: "mesh/grounded".to_string(),
                kind: RenderAssetKind::StaticMesh,
                content_hash: Some("cafe".to_string()),
                version: 1,
            },
        )]);

        let projected = EntityRenderProjector::new()
            .project(&state, &assets)
            .unwrap();
        assert!(matches!(
            &projected.frame.ops[0],
            RenderDiff::CreateStaticMeshInstance { instance, .. }
                if instance.transform.translation == [8.0, 0.0, -3.0]
        ));
        assert_eq!(state.world_transform(id), Some(world));
        assert!(state.collision(id).unwrap().enabled);
    }
}
