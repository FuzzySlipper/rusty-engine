use std::collections::BTreeMap;

use render_model::{
    MaterialUvStrategy, RenderDiff, RenderMaterialDescriptor, RenderMetadata, Transform,
};
use render_projection::{VoxelObjectProjectionInstance, VoxelObjectRenderProjector};
use voxel_asset::{
    with_computed_voxel_object_hashes, VoxelAssetBounds, VoxelAssetMaterialBinding,
    VoxelAssetMaterialMapping, VoxelCoordinateSystem, VoxelFrame, VoxelObjectAnimationFrame,
    VoxelObjectAsset, VoxelObjectClip, VoxelObjectGrid, VoxelObjectProvenance,
    VoxelObjectProvenanceKind, VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun,
    VOXEL_OBJECT_SCHEMA_VERSION,
};
use voxel_object_runtime::{admit_voxel_object, VoxelObjectRuntimeLimits};

#[test]
fn resource_instance_frame_swap_and_release_keep_the_handle_stable() {
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

    instances[0].frame = 1;
    let swapped = projector.project(&instances, &materials()).unwrap();
    assert_eq!(projector.handle("runner"), Some(handle));
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
    VoxelObjectProjectionInstance {
        instance_id: "runner".to_string(),
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
    let material = RenderMaterialDescriptor {
        schema_version: 1,
        id: "material/runner".to_string(),
        color: [0.8, 0.2, 0.1, 1.0],
        texture: None,
        roughness: 1.0,
        texture_tint: [1.0; 4],
        emission_color: [0.0; 3],
        emission_intensity: 0.0,
        uv_strategy: MaterialUvStrategy::Flat,
    };
    BTreeMap::from([(material.id.clone(), material)])
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
                    frame: default_frame,
                },
                VoxelObjectAnimationFrame {
                    duration_seconds: None,
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
