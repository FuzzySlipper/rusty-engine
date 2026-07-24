use core_assets::{AssetHash, AssetId, AssetReference, AssetVersionReq};
use core_ids::{EntityId, ProcessId, SceneId, SceneNodeId, TagId};
use core_math::Vec3;
use entity_state::{
    decode_snapshot, encode_durable_snapshot, encode_snapshot, ControllerCapability,
    EntityAuthoringService, EntityDefinition, EntityLifecycle, EntitySource, EntityState,
};

fn asset(text: &str) -> AssetReference {
    AssetReference::new(
        AssetId::parse(text).expect("asset id"),
        AssetVersionReq::Exact(3),
        Some(AssetHash::parse("00ff").expect("hash")),
    )
}

#[test]
fn schema_three_round_trip_preserves_source_labels_capabilities_and_relationships() {
    let parent = EntityId::new(1);
    let child = EntityId::new(2);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(parent, "scene-root")
            .with_source(EntitySource::AuthoredScene {
                scene: SceneId::new(7),
                node: SceneNodeId::new(8),
            })
            .with_labels([TagId::new(5)])
            .with_transform(Vec3::new(4.0, 0.0, 0.0))
            .with_bounds(Vec3::new(-1.0, -1.0, -1.0), Vec3::ONE)
            .with_asset_binding(asset("mesh/scene-root")),
        EntityDefinition::new(child, "child")
            .with_transform(Vec3::new(2.0, 0.0, 0.0))
            .with_collision(false, false)
            .with_controller(ControllerCapability::Process(ProcessId::new(9)))
            .with_transform_parent(parent)
            .with_containment(parent)
            .with_derivation(parent),
    ])
    .expect("fixture");
    EntityAuthoringService
        .disable(&mut state, 0, child)
        .expect("disabled");

    let encoded = encode_snapshot(&state).expect("encode");
    assert!(encoded.contains("\"schemaVersion\": 3"));
    let restored = decode_snapshot(&encoded).expect("decode");
    assert_eq!(restored.revision(), 1);
    assert_eq!(restored.view(parent), state.view(parent));
    assert_eq!(restored.view(child), state.view(child));
}

#[test]
fn durable_snapshot_drops_tooling_and_reroots_retained_children() {
    let tooling = EntityId::new(10);
    let child = EntityId::new(11);
    let state = EntityState::from_definitions([
        EntityDefinition::new(tooling, "gizmo")
            .with_source(EntitySource::DiagnosticTooling)
            .with_transform(Vec3::new(10.0, 0.0, 0.0)),
        EntityDefinition::new(child, "retained")
            .with_transform(Vec3::new(2.0, 0.0, 0.0))
            .with_transform_parent(tooling),
    ])
    .expect("fixture");
    let world_before = state.world_transform(child).unwrap();

    let encoded = encode_durable_snapshot(&state).expect("durable encode");
    let restored = decode_snapshot(&encoded).expect("durable decode");
    assert!(!restored.contains(tooling));
    assert_eq!(restored.world_transform(child), Some(world_before));
    assert_eq!(
        restored.relationships(child).unwrap().transform_parent,
        None
    );
}

#[test]
fn tombstones_round_trip_without_resurrecting_capabilities() {
    let id = EntityId::new(20);
    let mut state = EntityState::from_definitions([EntityDefinition::new(id, "spent")
        .with_transform(Vec3::ONE)
        .with_labels([TagId::new(3)])])
    .expect("fixture");
    EntityAuthoringService
        .destroy(&mut state, 0, id)
        .expect("destroyed");
    let restored = decode_snapshot(&encode_snapshot(&state).unwrap()).expect("decode");
    let view = restored.view(id).expect("tombstone");
    assert_eq!(view.lifecycle, EntityLifecycle::Tombstoned);
    assert_eq!(view.labels, vec![TagId::new(3)]);
    assert!(view.transform.is_none());
}

#[test]
fn strict_decode_rejects_nested_unknown_fields_and_trailing_input() {
    let state = EntityState::from_definitions([
        EntityDefinition::new(EntityId::new(30), "strict").with_transform(Vec3::ZERO)
    ])
    .unwrap();
    let encoded = encode_snapshot(&state).unwrap();
    let nested = encoded.replacen("\"scale\": [", "\"mystery\": true, \"scale\": [", 1);
    assert!(decode_snapshot(&nested).is_err());
    assert!(decode_snapshot(&(encoded + " trailing")).is_err());
}

#[test]
fn schema_two_snapshots_upgrade_with_explicit_defaults() {
    let legacy = r#"{
      "schemaVersion": 2,
      "revision": 4,
      "entities": [{
        "id": 40,
        "name": "legacy",
        "lifecycle": "disabled",
        "translation": [1.0, 2.0, 3.0],
        "collision": null,
        "renderable": null,
        "kinematic": null
      }]
    }"#;
    let state = decode_snapshot(legacy).expect("legacy migration");
    let view = state.view(EntityId::new(40)).unwrap();
    assert_eq!(state.revision(), 4);
    assert_eq!(view.lifecycle, EntityLifecycle::Disabled);
    assert_eq!(view.transform.unwrap().scale, Vec3::ONE);
    assert_eq!(view.source, EntitySource::RuntimeCreated { by: None });
}
