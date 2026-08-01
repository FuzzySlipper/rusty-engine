use std::collections::BTreeMap;

use render_model::{
    MaterialUvStrategy, MeshMaterialSlot, MeshPayloadSource, RenderDiff, RenderMaterialDescriptor,
    RenderMetadata, TextureFilter, TextureWrap, Transform, VoxelSurfaceAlphaModeDescriptor,
    VoxelSurfaceDescriptor, VoxelSurfaceMappingDescriptor,
};
use render_projection::{VoxelObjectProjectionInstance, VoxelObjectRenderProjector};
use voxel_asset::{
    with_computed_voxel_object_hashes, VoxelAssetBounds, VoxelAssetMaterialBinding,
    VoxelAssetMaterialMapping, VoxelCoordinateSystem, VoxelFrame, VoxelObjectAnimationFrame,
    VoxelObjectAsset, VoxelObjectClip, VoxelObjectGrid, VoxelObjectProvenance,
    VoxelObjectProvenanceKind, VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun,
    VOXEL_OBJECT_SCHEMA_VERSION,
};
use voxel_object_runtime::{
    admit_voxel_object, VoxelObjectLoopMode, VoxelObjectPlaybackStatus, VoxelObjectPlayer,
    VoxelObjectRuntimeLimits,
};

#[test]
fn frame_only_projection_reuses_cached_resource_without_materialization() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let mut instances = vec![instance(&object, 0)];
    let created = projector.project(&instances, &materials()).unwrap();
    let handle = projector.handle("runner").unwrap();
    assert!(matches!(
        created.frame.ops[0],
        RenderDiff::DefineMaterial { .. }
    ));
    assert!(created
        .frame
        .ops
        .iter()
        .any(|op| matches!(op, RenderDiff::DefineVoxelObject { .. })));
    assert!(created
        .frame
        .ops
        .iter()
        .any(|op| matches!(op, RenderDiff::CreateVoxelObjectInstance { .. })));
    assert_eq!(
        created.readout.materialized_resources,
        vec!["voxel-object/runner"]
    );

    instances[0].frame = 1;
    let swapped = projector.project(&instances, &materials()).unwrap();
    assert_eq!(projector.handle("runner"), Some(handle));
    assert!(swapped.readout.materialized_resources.is_empty());
    assert_eq!(swapped.frame.ops.len(), 1);
    assert!(matches!(
        swapped.frame.ops[0],
        RenderDiff::SetVoxelObjectFrame { frame: 1, .. }
    ));

    let released = projector.project(&[], &BTreeMap::new()).unwrap();
    assert!(matches!(released.frame.ops[0], RenderDiff::Destroy { .. }));
    assert!(matches!(
        released.frame.ops[1],
        RenderDiff::ReleaseVoxelObject { .. }
    ));
}

#[test]
fn scrubbed_player_frame_drives_renderer_neutral_presentation_only() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let mut instances = vec![instance(&object, 0)];
    projector.project(&instances, &materials()).unwrap();

    let mut player = VoxelObjectPlayer::new();
    player
        .scrub(&object, "walk", 1, VoxelObjectLoopMode::Repeat)
        .unwrap();
    let sample = player.sample_at(&object, 8_000_000).unwrap();
    assert_eq!(sample.status, VoxelObjectPlaybackStatus::Paused);
    assert_eq!(sample.clip_frame, Some(1));
    assert_eq!(sample.frame, 2);

    instances[0].frame = sample.frame;
    let presented = projector.project(&instances, &materials()).unwrap();
    assert_eq!(presented.frame.ops.len(), 1);
    assert!(matches!(
        presented.frame.ops[0],
        RenderDiff::SetVoxelObjectFrame { frame: 2, .. }
    ));
    assert!(presented.readout.materialized_resources.is_empty());
}

#[test]
fn shared_instances_materialize_one_resource() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let instances = vec![
        named_instance(&object, 0, "runner-a"),
        named_instance(&object, 1, "runner-b"),
    ];
    let created = projector.project(&instances, &materials()).unwrap();
    assert_eq!(
        created.readout.materialized_resources,
        vec!["voxel-object/runner"]
    );
    assert_eq!(
        created
            .frame
            .ops
            .iter()
            .filter(|operation| matches!(operation, RenderDiff::DefineVoxelObject { .. }))
            .count(),
        1
    );

    let unchanged = projector.project(&instances, &materials()).unwrap();
    assert!(unchanged.readout.materialized_resources.is_empty());
    assert!(unchanged.frame.ops.is_empty());
}

#[test]
fn packed_projection_moves_mesh_streams_out_of_the_control_frame() {
    let object = admitted();
    let instances = vec![instance(&object, 0)];
    let inline = VoxelObjectRenderProjector::new()
        .project(&instances, &materials())
        .unwrap();
    let mut packed_projector = VoxelObjectRenderProjector::with_packed_mesh_resources();
    let packed = packed_projector.project(&instances, &materials()).unwrap();

    assert_eq!(packed.mesh_resources.len(), 1);
    packed.mesh_resources[0].validate().unwrap();
    let render_asset = packed
        .frame
        .ops
        .iter()
        .find_map(|operation| match operation {
            RenderDiff::DefineVoxelObject { asset } => Some(asset),
            _ => None,
        })
        .unwrap();
    assert!(render_asset.meshes.iter().all(|mesh| matches!(
        mesh.payload.source,
        MeshPayloadSource::Resource {
            encoding: render_model::MeshResourceEncoding::PackedStreamsLeV2,
            uvs_byte_offset: Some(_),
            ..
        }
    )));
    assert!(
        serde_json::to_vec(&packed.frame).unwrap().len()
            < serde_json::to_vec(&inline.frame).unwrap().len()
    );

    let unchanged = packed_projector.project(&instances, &materials()).unwrap();
    assert!(unchanged.mesh_resources.is_empty());
    assert!(unchanged.frame.ops.is_empty());
}

#[test]
fn voxel_object_material_override_defines_the_selected_material() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let mut available_materials = materials();
    let override_material = material("material/override", [0.1, 0.4, 0.9, 1.0]);
    available_materials.insert(override_material.id.clone(), override_material);
    let mut projection_instance = instance(&object, 0);
    projection_instance.material_overrides = vec![MeshMaterialSlot {
        slot: 1,
        material: "material/override".to_string(),
    }];

    let projected = projector
        .project(&[projection_instance], &available_materials)
        .unwrap();
    let defined_materials = projected
        .frame
        .ops
        .iter()
        .filter_map(|operation| match operation {
            RenderDiff::DefineMaterial { material } => Some(material.id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        defined_materials,
        vec!["material/override", "material/runner"]
    );
    let created = projected
        .frame
        .ops
        .iter()
        .find_map(|operation| match operation {
            RenderDiff::CreateVoxelObjectInstance { instance, .. } => Some(instance),
            _ => None,
        })
        .unwrap();
    assert_eq!(created.material_overrides[0].material, "material/override");
}

#[test]
fn material_slot_can_switch_from_color_to_tile_without_voxel_republication() {
    let object = admitted();
    let instances = vec![instance(&object, 0)];
    let mut projector = VoxelObjectRenderProjector::new();
    projector.project(&instances, &materials()).unwrap();

    let mut tiled = material("material/runner", [0.8, 0.2, 0.1, 1.0]);
    tiled.texture = Some("texture/runner-tile".to_string());
    tiled.uv_strategy = MaterialUvStrategy::Planar;
    tiled.voxel_surface = Some(VoxelSurfaceDescriptor {
        schema_version: 1,
        filter: TextureFilter::Nearest,
        wrap: TextureWrap::Repeat,
        alpha_mode: VoxelSurfaceAlphaModeDescriptor::Opaque,
        mapping: VoxelSurfaceMappingDescriptor::Repeat {
            texture: "texture/runner-tile".to_string(),
            texture_version: 1,
            texture_content_hash: "aa01".to_string(),
            tile_scale_cells: [1.0, 1.0],
            tile_origin_cells: [0.0, 0.0],
        },
    });
    let updated = projector
        .project(
            &instances,
            &BTreeMap::from([(tiled.id.clone(), tiled.clone())]),
        )
        .unwrap();
    assert_eq!(
        updated.frame.ops,
        vec![RenderDiff::DefineMaterial { material: tiled }]
    );
    assert!(updated.readout.materialized_resources.is_empty());
}

#[test]
fn missing_voxel_object_material_override_is_fail_atomic() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let mut projection_instance = instance(&object, 0);
    projection_instance.material_overrides = vec![MeshMaterialSlot {
        slot: 1,
        material: "material/missing".to_string(),
    }];

    let error = projector
        .project(&[projection_instance], &materials())
        .unwrap_err();
    assert!(matches!(
        error,
        render_projection::VoxelObjectProjectionError::MissingMaterial { asset }
            if asset == "material/missing"
    ));
    assert_eq!(projector.handle("runner"), None);
}

#[test]
fn invalid_frame_is_fail_atomic() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    projector
        .project(&[instance(&object, 0)], &materials())
        .unwrap();
    let handle = projector.handle("runner").unwrap();
    let error = projector
        .project(&[instance(&object, 99)], &materials())
        .unwrap_err();
    assert!(matches!(
        error,
        render_projection::VoxelObjectProjectionError::FrameOutOfRange { .. }
    ));
    assert_eq!(projector.handle("runner"), Some(handle));
}

fn instance(
    object: &voxel_object_runtime::AdmittedVoxelObject,
    frame: u32,
) -> VoxelObjectProjectionInstance<'_> {
    named_instance(object, frame, "runner")
}

fn named_instance<'a>(
    object: &'a voxel_object_runtime::AdmittedVoxelObject,
    frame: u32,
    instance_id: &str,
) -> VoxelObjectProjectionInstance<'a> {
    VoxelObjectProjectionInstance {
        instance_id: instance_id.to_string(),
        object,
        frame,
        transform: Transform::IDENTITY,
        visible: true,
        material_overrides: Vec::new(),
        metadata: RenderMetadata {
            source_entity: Some(7),
            source_scene_node: None,
            tags: vec!["voxel-object".to_string()],
            label: Some("Runner".to_string()),
        },
    }
}

fn materials() -> BTreeMap<String, RenderMaterialDescriptor> {
    let material = material("material/runner", [0.8, 0.2, 0.1, 1.0]);
    BTreeMap::from([(material.id.clone(), material)])
}

fn material(id: &str, color: [f32; 4]) -> RenderMaterialDescriptor {
    RenderMaterialDescriptor {
        schema_version: 1,
        id: id.to_string(),
        color,
        texture: None,
        roughness: 1.0,
        texture_tint: [1.0; 4],
        emission_color: [0.0; 3],
        emission_intensity: 0.0,
        uv_strategy: MaterialUvStrategy::Flat,
        voxel_surface: None,
    }
}

fn admitted() -> voxel_object_runtime::AdmittedVoxelObject {
    admit_voxel_object(&object(), VoxelObjectRuntimeLimits::default()).unwrap()
}

fn object() -> VoxelObjectAsset {
    let default_frame = frame([0, 0, 0]);
    with_computed_voxel_object_hashes(VoxelObjectAsset {
        schema_version: VOXEL_OBJECT_SCHEMA_VERSION,
        asset_id: "voxel-object/runner".to_string(),
        grid: VoxelObjectGrid {
            coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
            cell_size: 1.0,
            chunk_size: 16,
            pivot: [0.0; 3],
        },
        bounds: VoxelAssetBounds {
            min: [0, 0, 0],
            max: [1, 0, 0],
        },
        default_frame: default_frame.clone(),
        clips: vec![VoxelObjectClip {
            id: "walk".to_string(),
            name: None,
            frames_per_second: 10.0,
            frames: vec![
                VoxelObjectAnimationFrame {
                    duration_seconds: None,
                    anchors: Vec::new(),
                    collision: None,
                    frame: default_frame,
                },
                VoxelObjectAnimationFrame {
                    duration_seconds: None,
                    anchors: Vec::new(),
                    collision: None,
                    frame: frame([1, 0, 0]),
                },
            ],
        }],
        default_clip: Some("walk".to_string()),
        material_palette: vec![VoxelAssetMaterialBinding {
            material_slot: 1,
            material_asset_id: "material/runner".to_string(),
            display_name: None,
        }],
        material_map: vec![VoxelAssetMaterialMapping {
            source_material_slot: 0,
            source_material_name: None,
            voxel_material_slot: 1,
        }],
        provenance: VoxelObjectProvenance {
            kind: VoxelObjectProvenanceKind::Authored,
            source_path: "runner.voxel.json".to_string(),
            source_sha256:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            source_byte_count: 1,
            converter: "test".to_string(),
            settings_sha256:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            license_path: None,
            source_clips: vec![],
        },
        content_hash: String::new(),
    })
    .unwrap()
}

fn frame(coordinate: [i64; 3]) -> VoxelFrame {
    VoxelFrame {
        bounds: VoxelAssetBounds {
            min: coordinate,
            max: coordinate,
        },
        representation: VoxelRepresentation {
            kind: VoxelRepresentationKind::SparseRuns,
            sparse_runs: vec![VoxelSparseRun {
                start: coordinate,
                length: 1,
                material_slot: 1,
            }],
        },
        voxel_data_hash: String::new(),
    }
}
