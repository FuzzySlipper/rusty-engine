use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use voxel_asset::{decode_voxel_asset, VoxelConversionMode};
use voxel_convert::{
    convert_and_install, convert_glb, decode_conversion_request, import_static_glb,
};

const SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../asha-engine/harness/fixtures/voxel-conversion/kenney-wall-a.glb"
));
const REQUEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../content/conversion/kenney-wall-a.request.json"
));
const ARTIFACT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../content/assets/kenney-wall-a.voxel.json"
));

#[test]
fn real_glb_import_is_bounded_static_geometry_with_materials() {
    let mesh = import_static_glb(SOURCE).expect("real static GLB");
    assert_eq!(mesh.positions.len(), 48);
    assert_eq!(mesh.triangles.len(), 12);
    assert_eq!(mesh.materials.len(), 2);
    assert_eq!(mesh.materials[0].source_material_slot, 0);
    assert_eq!(
        mesh.materials[0].source_material_name.as_deref(),
        Some("wall_lines")
    );
    assert_eq!(mesh.materials[1].source_material_slot, 1);
    assert_eq!(
        mesh.materials[1].source_material_name.as_deref(),
        Some("concrete")
    );
}

#[test]
fn real_conversion_is_byte_reproducible_and_matches_checked_artifact() {
    let request = decode_conversion_request(REQUEST).unwrap();
    let first = convert_glb(&request, SOURCE).unwrap();
    let second = convert_glb(&request, SOURCE).unwrap();

    assert_eq!(first.canonical_json, second.canonical_json);
    assert_eq!(first.content_hash, second.content_hash);
    assert_eq!(first.canonical_json, ARTIFACT);
    assert_eq!(first.source_vertices, 48);
    assert_eq!(first.source_triangles, 12);
    assert_eq!(first.output_voxels, 8);
    assert_eq!(first.sparse_runs, 4);
    assert_eq!(decode_voxel_asset(ARTIFACT).unwrap(), first.asset);
}

#[test]
fn settings_variation_changes_canonical_artifact_and_stale_identity_fails() {
    let request = decode_conversion_request(REQUEST).unwrap();
    let baseline = convert_glb(&request, SOURCE).unwrap();

    let mut varied = request.clone();
    varied.settings.material_map[0].voxel_material_slot = 9;
    let varied = convert_glb(&varied, SOURCE).unwrap();
    assert_ne!(varied.content_hash, baseline.content_hash);
    assert_ne!(varied.canonical_json, baseline.canonical_json);
    assert_ne!(varied.asset.representation, baseline.asset.representation);

    let mut stale = request;
    stale.expected_source_sha256 =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
    let error = convert_glb(&stale, SOURCE).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.sourceHashMismatch");
}

#[test]
fn malformed_sources_material_gaps_and_unsupported_solid_topology_fail_closed() {
    let request = decode_conversion_request(REQUEST).unwrap();
    let mut malformed_request = request.clone();
    malformed_request.expected_source_sha256 = format!("sha256:{:x}", Sha256::digest(b"not a glb"));
    let malformed = convert_glb(&malformed_request, b"not a glb").unwrap_err();
    assert_eq!(malformed.diagnostics()[0].code, "conversion.invalidSource");

    let mut missing_material = request.clone();
    missing_material.settings.material_map.pop();
    let error = convert_glb(&missing_material, SOURCE).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        "conversion.materialMapMismatch"
    );

    let mut solid = request;
    solid.settings.mode = VoxelConversionMode::Solid;
    let error = convert_glb(&solid, SOURCE).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        "conversion.unsupportedTopology"
    );
}

#[test]
fn failed_conversion_never_replaces_a_known_good_artifact() {
    let request = decode_conversion_request(REQUEST).unwrap();
    let directory = temporary_directory();
    fs::create_dir(&directory).unwrap();
    let output = directory.join("wall.voxel.json");
    fs::write(&output, "known-good\n").unwrap();

    let mut stale = request;
    stale.expected_source_sha256 =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
    assert!(convert_and_install(&stale, SOURCE, &output).is_err());
    assert_eq!(fs::read_to_string(&output).unwrap(), "known-good\n");
    assert!(!directory.join("wall.voxel.json.pending").exists());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn request_decode_is_strict_and_source_locatable() {
    let mut value: serde_json::Value = serde_json::from_str(REQUEST).unwrap();
    value["settings"]["unexpected"] = true.into();
    let error = decode_conversion_request(&serde_json::to_string(&value).unwrap()).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.requestDecode");
    assert!(error.diagnostics()[0].path.starts_with("settings"));
}

fn temporary_directory() -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "rusty-engine-voxel-convert-{}-{unique}",
        std::process::id()
    ))
}
