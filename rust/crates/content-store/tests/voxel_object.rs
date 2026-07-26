use content_store::{
    decode_manifest, encode_manifest, voxel_object_body, ArtifactRole, ContentArtifact,
    ContentLoadPlan, ContentLoadStage, ContentManifest,
};
use voxel_asset::{
    decode_voxel_object, with_computed_voxel_object_hashes, VoxelAssetBounds,
    VoxelAssetMaterialBinding, VoxelAssetMaterialMapping, VoxelCoordinateSystem, VoxelFrame,
    VoxelObjectAsset, VoxelObjectGrid, VoxelObjectProvenance, VoxelObjectProvenanceKind,
    VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun, VOXEL_OBJECT_SCHEMA_VERSION,
};

#[test]
fn voxel_objects_are_canonical_owner_bytes_loaded_as_asset_data() {
    let object =
        with_computed_voxel_object_hashes(VoxelObjectAsset {
            schema_version: VOXEL_OBJECT_SCHEMA_VERSION,
            asset_id: "voxel-object/crate".to_owned(),
            grid: VoxelObjectGrid {
                coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
                cell_size: 1.0,
                chunk_size: 8,
                pivot: [0.5, 0.0, 0.5],
            },
            bounds: VoxelAssetBounds {
                min: [0, 0, 0],
                max: [0, 0, 0],
            },
            default_frame: VoxelFrame {
                bounds: VoxelAssetBounds {
                    min: [0, 0, 0],
                    max: [0, 0, 0],
                },
                representation: VoxelRepresentation {
                    kind: VoxelRepresentationKind::SparseRuns,
                    sparse_runs: vec![VoxelSparseRun {
                        start: [0, 0, 0],
                        length: 1,
                        material_slot: 1,
                    }],
                },
                voxel_data_hash: String::new(),
            },
            clips: vec![],
            default_clip: None,
            material_palette: vec![VoxelAssetMaterialBinding {
                material_slot: 1,
                material_asset_id: "material/stone".to_owned(),
                display_name: Some("Stone".to_owned()),
            }],
            material_map: vec![VoxelAssetMaterialMapping {
                source_material_slot: 0,
                source_material_name: Some("stone".to_owned()),
                voxel_material_slot: 1,
            }],
            provenance: VoxelObjectProvenance {
                kind: VoxelObjectProvenanceKind::Authored,
                source_path: "objects/crate.voxel-object.json".to_owned(),
                source_sha256:
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_owned(),
                source_byte_count: 1,
                converter: "rusty-engine.voxel-object.authoring.v1".to_owned(),
                settings_sha256:
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_owned(),
                license_path: None,
            },
            content_hash: String::new(),
        })
        .unwrap();
    let body = voxel_object_body("objects/crate.voxel-object.json", &object).unwrap();
    assert_eq!(
        decode_voxel_object(std::str::from_utf8(&body.bytes).unwrap()).unwrap(),
        object
    );

    let manifest = ContentManifest::new(vec![ContentArtifact::durable(
        &body.path,
        ArtifactRole::VoxelObject,
        &body.bytes,
    )]);
    let encoded = encode_manifest(&manifest).unwrap();
    assert!(encoded.contains("\"role\": \"voxelObject\""));
    let decoded = decode_manifest(&encoded).unwrap();
    let plan = ContentLoadPlan::build(&decoded).unwrap();
    assert_eq!(plan.steps[0].stage, ContentLoadStage::AssetData);
}
