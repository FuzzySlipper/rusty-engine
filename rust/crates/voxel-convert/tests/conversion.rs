use std::collections::BTreeSet;
use std::fs::{self, File};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use voxel_asset::{
    decode_voxel_asset, VoxelAssetMaterialBinding, VoxelAssetMaterialMapping, VoxelConversionMode,
};
use voxel_convert::{
    convert_and_install, convert_glb, decode_conversion_request, import_static_glb,
    import_static_glb_scene, source_sha256, MAX_CONVERSION_REQUEST_BYTES,
    MAX_CONVERSION_SOURCE_BYTES,
};

const SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/voxel-conversion/kenney-wall-a.glb"
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
    assert_eq!(mesh.texture_coordinates.len(), 1);
    assert_eq!(mesh.texture_coordinates[0].source_set_index, 0);
    assert!(mesh.texture_coordinates[0]
        .coordinates
        .iter()
        .all(Option::is_some));
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
fn transformed_and_multiply_instanced_meshes_compose_deterministically() {
    let transformed = mutate_glb_json(|document| {
        document["nodes"][0]["translation"] = serde_json::json!([1.0, 0.0, 0.0]);
    });
    let transformed_mesh = import_static_glb(&transformed).unwrap();
    assert_eq!(transformed_mesh.positions[0], [0.5, 0.0, -0.5]);

    let instanced = mutate_glb_json(|document| {
        let mut second = document["nodes"][0].clone();
        second["name"] = "wall-b".into();
        second["translation"] = serde_json::json!([2.0, 0.0, 0.0]);
        document["nodes"].as_array_mut().unwrap().push(second);
        document["scenes"][0]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(1.into());
    });
    let scene = import_static_glb_scene(&instanced).unwrap();
    let mesh = import_static_glb(&instanced).unwrap();
    assert_eq!(scene.nodes.len(), 2);
    assert_eq!(scene.meshes.len(), 1);
    assert_eq!(mesh.positions.len(), 96);
    assert_eq!(mesh.triangles.len(), 24);
    assert_eq!(mesh.primitive_groups.len(), 4);
    assert_eq!(
        mesh.primitive_groups
            .iter()
            .map(|group| (
                group.source_node_index,
                group.source_mesh_index,
                group.source_primitive_index
            ))
            .collect::<Vec<_>>(),
        vec![(0, 0, 0), (0, 0, 1), (1, 0, 0), (1, 0, 1)]
    );
    assert_eq!(mesh, import_static_glb(&instanced).unwrap());
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
    assert_eq!(first.voxelization_work, 48);
    assert_eq!(first.output_voxels, 8);
    assert_eq!(
        first.asset.provenance.converter,
        "rusty-engine.mesh-to-voxel.v2"
    );
    assert_eq!(first.sparse_runs, 4);
    assert_eq!(decode_voxel_asset(ARTIFACT).unwrap(), first.asset);
}

#[test]
fn settings_variation_changes_canonical_artifact_and_stale_identity_fails() {
    let request = decode_conversion_request(REQUEST).unwrap();
    let baseline = convert_glb(&request, SOURCE).unwrap();

    let mut varied = request.clone();
    varied.settings.material_map[0].voxel_material_slot = 9;
    varied.settings.material_palette[0].material_slot = 9;
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

    let open_source = seam_split_tetrahedron_glb(3);
    let open_request = seam_split_tetrahedron_request(&open_source, 3);
    let error = convert_glb(&open_request, &open_source).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        "conversion.unsupportedTopology"
    );
}

#[test]
fn solid_accepts_closed_geometry_with_vertex_splits_at_face_seams() {
    let source = seam_split_tetrahedron_glb(4);
    let mesh = import_static_glb(&source).unwrap();
    assert_eq!(mesh.positions.len(), 12);
    assert_eq!(mesh.triangles.len(), 4);
    assert_eq!(mesh.primitive_groups.len(), 4);
    assert_eq!(mesh.materials.len(), 4);

    let request = seam_split_tetrahedron_request(&source, 4);

    let first = convert_glb(&request, &source).unwrap();
    let second = convert_glb(&request, &source).unwrap();
    assert_eq!(first.canonical_json, second.canonical_json);
    assert_eq!(first.content_hash, second.content_hash);
    assert!(first.output_voxels > 0);
    assert_eq!(
        first
            .asset
            .representation
            .sparse_runs
            .iter()
            .map(|run| run.material_slot)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([1, 2, 3, 4])
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

fn seam_split_tetrahedron_request(
    source: &[u8],
    face_count: u16,
) -> voxel_asset::VoxelConversionRequest {
    let mut request = decode_conversion_request(REQUEST).unwrap();
    request.asset_id = "voxel-volume/seam-split-tetrahedron".to_owned();
    request.source_path = "generated/seam-split-tetrahedron.glb".to_owned();
    request.expected_source_sha256 = source_sha256(source);
    request.settings.mode = VoxelConversionMode::Solid;
    request.settings.resolution = [13, 13, 13];
    request.settings.max_output_voxels = 10_000;
    request.settings.material_palette = (0..face_count)
        .map(|slot| VoxelAssetMaterialBinding {
            material_slot: slot + 1,
            material_asset_id: format!("material/seam-face-{slot}"),
            display_name: None,
        })
        .collect();
    request.settings.material_map = (0..face_count)
        .map(|slot| VoxelAssetMaterialMapping {
            source_material_slot: u32::from(slot),
            source_material_name: Some(format!("face-{slot}")),
            voxel_material_slot: slot + 1,
        })
        .collect();
    request
}

fn seam_split_tetrahedron_glb(face_count: usize) -> Vec<u8> {
    let faces = [
        [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
        [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        [[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]],
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    ];
    let texture_coordinates = [[0.0f32, 0.0], [1.0, 0.0], [0.0, 1.0]];
    let mut binary = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();
    let mut primitives = Vec::new();

    assert!((1..=faces.len()).contains(&face_count));
    for (face_index, face) in faces.iter().take(face_count).enumerate() {
        let positions = face.map(|position| position.map(|component| component as f32));
        let position_view = push_buffer_view(
            &mut binary,
            &mut buffer_views,
            &encode_f32_components(&positions),
            34_962,
        );
        let position_accessor = accessors.len();
        accessors.push(serde_json::json!({
            "bufferView": position_view,
            "componentType": 5126,
            "count": 3,
            "type": "VEC3",
            "min": [
                face.iter().map(|position| position[0]).fold(f64::INFINITY, f64::min),
                face.iter().map(|position| position[1]).fold(f64::INFINITY, f64::min),
                face.iter().map(|position| position[2]).fold(f64::INFINITY, f64::min),
            ],
            "max": [
                face.iter().map(|position| position[0]).fold(f64::NEG_INFINITY, f64::max),
                face.iter().map(|position| position[1]).fold(f64::NEG_INFINITY, f64::max),
                face.iter().map(|position| position[2]).fold(f64::NEG_INFINITY, f64::max),
            ],
        }));

        let texture_view = push_buffer_view(
            &mut binary,
            &mut buffer_views,
            &encode_f32_components(&texture_coordinates),
            34_962,
        );
        let texture_accessor = accessors.len();
        accessors.push(serde_json::json!({
            "bufferView": texture_view,
            "componentType": 5126,
            "count": 3,
            "type": "VEC2",
        }));

        let index_view = push_buffer_view(
            &mut binary,
            &mut buffer_views,
            &encode_u16_components(&[0, 1, 2]),
            34_963,
        );
        let index_accessor = accessors.len();
        accessors.push(serde_json::json!({
            "bufferView": index_view,
            "componentType": 5123,
            "count": 3,
            "type": "SCALAR",
        }));
        primitives.push(serde_json::json!({
            "attributes": {
                "POSITION": position_accessor,
                "TEXCOORD_0": texture_accessor,
            },
            "indices": index_accessor,
            "material": face_index,
            "mode": 4,
        }));
    }
    pad_to_four(&mut binary, 0);

    let document = serde_json::json!({
        "asset": {"version": "2.0", "generator": "rusty-engine seam topology test"},
        "scene": 0,
        "scenes": [{"name": "seam-split-tetrahedron", "nodes": [0]}],
        "nodes": [{"name": "tetrahedron", "mesh": 0}],
        "meshes": [{"name": "four-face-seams", "primitives": primitives}],
        "materials": (0..face_count).map(|index| serde_json::json!({"name": format!("face-{index}")})).collect::<Vec<_>>(),
        "buffers": [{"byteLength": binary.len()}],
        "bufferViews": buffer_views,
        "accessors": accessors,
    });
    encode_glb(document, binary)
}

fn push_buffer_view(
    binary: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    bytes: &[u8],
    target: u32,
) -> usize {
    pad_to_four(binary, 0);
    let byte_offset = binary.len();
    binary.extend_from_slice(bytes);
    let index = buffer_views.len();
    buffer_views.push(serde_json::json!({
        "buffer": 0,
        "byteOffset": byte_offset,
        "byteLength": bytes.len(),
        "target": target,
    }));
    index
}

fn encode_f32_components<const WIDTH: usize>(values: &[[f32; WIDTH]]) -> Vec<u8> {
    values
        .iter()
        .flatten()
        .flat_map(|component| component.to_le_bytes())
        .collect()
}

fn encode_u16_components(values: &[u16]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|component| component.to_le_bytes())
        .collect()
}

fn encode_glb(document: serde_json::Value, binary: Vec<u8>) -> Vec<u8> {
    let mut json = serde_json::to_vec(&document).unwrap();
    pad_to_four(&mut json, b' ');
    let total_length = 12 + 8 + json.len() + 8 + binary.len();
    let mut glb = Vec::with_capacity(total_length);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total_length as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&binary);
    glb
}

fn pad_to_four(bytes: &mut Vec<u8>, padding: u8) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(padding);
    }
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
