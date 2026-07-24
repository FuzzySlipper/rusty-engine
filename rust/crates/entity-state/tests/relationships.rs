use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{
    EntityAuthoringService, EntityDefinition, EntityState, RelationshipCommand, RelationshipError,
    RelationshipKind, TransformParentMode,
};

fn fixture() -> EntityState {
    EntityState::from_definitions([
        EntityDefinition::new(EntityId::new(1), "root").with_transform(Vec3::new(10.0, 0.0, 0.0)),
        EntityDefinition::new(EntityId::new(2), "child").with_transform(Vec3::new(2.0, 0.0, 0.0)),
        EntityDefinition::new(EntityId::new(3), "leaf").with_transform(Vec3::new(1.0, 0.0, 0.0)),
    ])
    .expect("fixture")
}

#[test]
fn relationship_preview_is_read_only_and_apply_is_revision_guarded() {
    let mut state = fixture();
    let command = RelationshipCommand::SetTransformParent {
        child: EntityId::new(2),
        parent: EntityId::new(1),
        mode: TransformParentMode::KeepLocal,
    };
    let preview = state.preview_relationship(command).expect("preview");
    assert!(preview.changes_state);
    assert_eq!(state.revision(), 0);
    assert_eq!(
        state
            .relationships(EntityId::new(2))
            .unwrap()
            .transform_parent,
        None
    );

    state.apply_relationship(0, command).expect("apply");
    assert_eq!(state.revision(), 1);
    assert_eq!(
        state
            .world_transform(EntityId::new(2))
            .expect("world transform")
            .translation,
        Vec3::new(12.0, 0.0, 0.0)
    );
    assert!(matches!(
        state.apply_relationship(
            0,
            RelationshipCommand::ClearTransformParent {
                child: EntityId::new(2),
                keep_world: true,
            }
        ),
        Err(RelationshipError::StaleRevision { .. })
    ));
}

#[test]
fn cycles_are_rejected_without_partial_mutation() {
    let mut state = fixture();
    state
        .apply_relationship(
            0,
            RelationshipCommand::SetContainment {
                child: EntityId::new(2),
                container: EntityId::new(1),
            },
        )
        .expect("first link");
    let error = state
        .apply_relationship(
            1,
            RelationshipCommand::SetContainment {
                child: EntityId::new(1),
                container: EntityId::new(2),
            },
        )
        .expect_err("cycle rejected");
    assert!(matches!(
        error,
        RelationshipError::Cycle {
            kind: RelationshipKind::Containment,
            ..
        }
    ));
    assert_eq!(state.revision(), 1);
    assert_eq!(
        state.relationships(EntityId::new(1)).unwrap().contained_in,
        None
    );
}

#[test]
fn keep_world_and_parent_destruction_reroot_children() {
    let mut state = fixture();
    state
        .apply_relationship(
            0,
            RelationshipCommand::SetTransformParent {
                child: EntityId::new(2),
                parent: EntityId::new(1),
                mode: TransformParentMode::KeepLocal,
            },
        )
        .expect("parented");
    let world_before = state.world_transform(EntityId::new(2)).unwrap();
    state
        .apply_relationship(
            1,
            RelationshipCommand::ClearTransformParent {
                child: EntityId::new(2),
                keep_world: true,
            },
        )
        .expect("rerooted");
    assert_eq!(state.world_transform(EntityId::new(2)), Some(world_before));

    state
        .apply_relationship(
            2,
            RelationshipCommand::SetTransformParent {
                child: EntityId::new(2),
                parent: EntityId::new(1),
                mode: TransformParentMode::KeepLocal,
            },
        )
        .expect("parented again");
    let world_before_destroy = state.world_transform(EntityId::new(2)).unwrap();
    EntityAuthoringService
        .destroy(&mut state, 3, EntityId::new(1))
        .expect("parent destroyed");
    assert_eq!(
        state.world_transform(EntityId::new(2)),
        Some(world_before_destroy)
    );
    assert_eq!(
        state
            .relationships(EntityId::new(2))
            .unwrap()
            .transform_parent,
        None
    );
}

#[test]
fn render_grouping_remains_a_projection_concern() {
    let state = fixture();
    assert_eq!(
        state
            .preview_relationship(RelationshipCommand::SetRenderGroup {
                entity: EntityId::new(1),
                group: EntityId::new(2),
            })
            .expect_err("render grouping is not authoritative state"),
        RelationshipError::ProjectionOnly {
            kind: RelationshipKind::RenderGrouping
        }
    );
}

#[test]
fn source_ancestry_survives_origin_destruction_as_provenance() {
    let mut state = fixture();
    state
        .apply_relationship(
            0,
            RelationshipCommand::SetSourceAncestry {
                entity: EntityId::new(2),
                source: EntityId::new(1),
            },
        )
        .expect("source recorded");
    EntityAuthoringService
        .destroy(&mut state, 1, EntityId::new(1))
        .expect("origin destroyed");
    assert_eq!(
        state.relationships(EntityId::new(2)).unwrap().derived_from,
        Some(EntityId::new(1))
    );
}
