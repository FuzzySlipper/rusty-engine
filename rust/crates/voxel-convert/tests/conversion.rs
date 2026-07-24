use std::fs::{self, File};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use voxel_asset::{decode_voxel_asset, VoxelConversionMode};
use voxel_convert::{
    convert_and_install, convert_glb, decode_conversion_request, import_static_glb,
    MAX_CONVERSION_REQUEST_BYTES, MAX_CONVERSION_SOURCE_BYTES,
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
fn transformed_and_multiply_instanced_meshes_fail_closed() {
    let transformed = mutate_glb_json(|document| {
        document["nodes"][0]["translation"] = serde_json::json!([1.0, 0.0, 0.0]);
    });
    let error = import_static_glb(&transformed).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.unsupportedFeature");
    assert_eq!(error.diagnostics()[0].path, "source.nodes[0].transform");

    let instanced = mutate_glb_json(|document| {
        let second = document["nodes"][0].clone();
        document["nodes"].as_array_mut().unwrap().push(second);
        document["scenes"][0]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(1.into());
    });
    let error = import_static_glb(&instanced).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.unsupportedFeature");
    assert_eq!(error.diagnostics()[0].path, "source.nodes");
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

#[test]
fn cli_bounds_request_and_sparse_source_before_conversion() {
    let directory = temporary_directory();
    fs::create_dir(&directory).unwrap();
    let request_path = directory.join("request.json");
    let source_path = directory.join("source.glb");
    let output_path = directory.join("known-good.voxel.json");
    fs::write(&source_path, SOURCE).unwrap();
    fs::write(&output_path, "known-good\n").unwrap();

    fs::write(&request_path, vec![b' '; MAX_CONVERSION_REQUEST_BYTES + 1]).unwrap();
    let oversized_request = run_cli(&request_path, &source_path, &output_path);
    assert!(!oversized_request.status.success());
    assert!(String::from_utf8_lossy(&oversized_request.stderr).contains("conversion.resourceLimit"));
    assert_eq!(fs::read_to_string(&output_path).unwrap(), "known-good\n");

    fs::write(&request_path, REQUEST).unwrap();
    let sparse_source = File::create(&source_path).unwrap();
    sparse_source
        .set_len(MAX_CONVERSION_SOURCE_BYTES + 1)
        .unwrap();
    let oversized_source = run_cli(&request_path, &source_path, &output_path);
    assert!(!oversized_source.status.success());
    assert!(String::from_utf8_lossy(&oversized_source.stderr).contains("conversion.resourceLimit"));
    assert_eq!(fs::read_to_string(&output_path).unwrap(), "known-good\n");
    assert!(!directory.join("known-good.voxel.json.pending").exists());

    fs::remove_dir_all(directory).unwrap();
}

fn run_cli(
    request: &std::path::Path,
    source: &std::path::Path,
    output: &std::path::Path,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_voxel-convert"))
        .arg("--request")
        .arg(request)
        .arg("--source")
        .arg(source)
        .arg("--output")
        .arg(output)
        .output()
        .expect("run voxel-convert CLI")
}

fn mutate_glb_json(change: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    assert_eq!(&SOURCE[0..4], b"glTF");
    let json_length = u32::from_le_bytes(SOURCE[12..16].try_into().unwrap()) as usize;
    assert_eq!(&SOURCE[16..20], b"JSON");
    let json_end = 20 + json_length;
    let mut document: serde_json::Value = serde_json::from_slice(&SOURCE[20..json_end]).unwrap();
    change(&mut document);
    let mut json = serde_json::to_vec(&document).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }

    let total_length = 20 + json.len() + (SOURCE.len() - json_end);
    let mut glb = Vec::with_capacity(total_length);
    glb.extend_from_slice(&SOURCE[0..8]);
    glb.extend_from_slice(&(total_length as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&SOURCE[json_end..]);
    glb
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
