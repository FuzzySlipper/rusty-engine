use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use voxel_asset::{
    decode_voxel_asset, VoxelAssetBounds, VoxelConversionFitPolicy, VoxelConversionMode,
    VoxelConversionOriginPolicy,
};
use voxel_convert::{
    apply_conversion, apply_conversion_and_install, conversion_plan_hash,
    decode_conversion_request, decode_mesh_source_import_request, identity_transform,
    import_mesh_source, plan_conversion, plan_settings_sha256, preview_conversion,
    query_model_info, query_model_window, texture_coordinate_source_hash, ConversionApplyRequest,
    ConversionMaterialPolicy, ConversionPlanRequest, ConversionPlanSettings,
    ConversionPreviewRequest, MeshSourceFormat, MeshSourceImportRequest, TextureChannelLayout,
    TextureColorSpace, TextureMaterialBinding, TextureMaterialMode, TextureSampleAsset,
    TextureSamplingPolicy, TextureSourceRef, TextureUvAttributeRef, TextureWrapPolicy,
    VoxelModelInfoRequest, VoxelModelWindowRequest, MAX_CONVERSION_RESOLUTION_AXIS,
    MAX_MESH_SOURCE_PATH_BYTES,
};

const SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/voxel-conversion/kenney-wall-a.glb"
));
const REQUEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../content/conversion/kenney-wall-a.request.json"
));
const SOURCE_HASH: &str = "sha256:6fceda24c30d2c22694f232f03fe2115fb1a462046fbbf719a90eea10dc9af00";

#[test]
fn imported_source_has_hash_pinned_groups_materials_and_strict_shape() {
    let request = import_request();
    let imported = import_mesh_source(&request).expect("bounded imported source");
    assert_eq!(imported.receipt.source.source_sha256, SOURCE_HASH);
    assert_eq!(imported.receipt.metadata.vertex_count, 48);
    assert_eq!(imported.receipt.metadata.triangle_count, 12);
    assert_eq!(imported.receipt.metadata.groups.len(), 2);
    assert_eq!(imported.receipt.metadata.material_slots.len(), 2);
    assert_eq!(
        imported
            .receipt
            .metadata
            .groups
            .iter()
            .map(|group| group.index_count)
            .sum::<u32>(),
        36
    );
    assert!(imported
        .receipt
        .metadata
        .groups
        .iter()
        .all(|group| group.index_count > 0 && group.index_count.is_multiple_of(3)));

    let encoded = serde_json::to_string(&request).unwrap();
    assert_eq!(
        decode_mesh_source_import_request(&encoded).unwrap(),
        request
    );
    let mut unknown: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    unknown["unknown"] = true.into();
    let error =
        decode_mesh_source_import_request(&serde_json::to_string(&unknown).unwrap()).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.requestDecode");

    let mut stale = request.clone();
    stale.expected_source_sha256 = Some(hash('a'));
    let error = import_mesh_source(&stale).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.sourceHashMismatch");

    let mut oversized_path = request;
    oversized_path.source_path = "p".repeat(MAX_MESH_SOURCE_PATH_BYTES + 1);
    let error = import_mesh_source(&oversized_path).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.invalidString");
}

#[test]
fn adjacent_primitives_keep_distinct_groups_when_they_share_a_material() {
    let mut request = import_request();
    request.source_bytes = glb_with_shared_primitive_material();
    request.expected_source_sha256 = None;
    let imported = import_mesh_source(&request).expect("shared-material primitives import");

    assert_eq!(imported.receipt.metadata.groups.len(), 2);
    assert_eq!(
        imported.receipt.metadata.groups[0].source_material_slot,
        imported.receipt.metadata.groups[1].source_material_slot
    );
    assert_ne!(
        imported.receipt.metadata.groups[0].group_id,
        imported.receipt.metadata.groups[1].group_id
    );
    assert_eq!(
        imported
            .receipt
            .metadata
            .groups
            .iter()
            .map(|group| group.index_count)
            .sum::<u32>(),
        imported.receipt.metadata.triangle_count * 3
    );
}

#[test]
fn plan_preview_apply_are_hash_guarded_bounded_and_transform_aware() {
    let imported = imported_source();
    let baseline_request = plan_request(&imported);
    let baseline = plan_conversion(&baseline_request, &imported).unwrap();
    assert_eq!(
        baseline.plan().settings_sha256,
        plan_settings_sha256(&baseline_request.settings)
    );
    assert_eq!(
        baseline.candidate().asset.provenance.settings_sha256,
        baseline.plan().settings_sha256
    );

    let preview = preview_conversion(
        &ConversionPreviewRequest {
            plan_id: baseline.plan().plan_id.clone(),
            expected_plan_hash: baseline.plan().plan_hash.clone(),
            max_samples: 2,
        },
        &baseline,
    )
    .unwrap();
    assert_eq!(preview.sample_voxels.len(), 2);
    assert!(preview.samples_truncated);

    let applied = apply_conversion(
        &ConversionApplyRequest {
            plan_id: baseline.plan().plan_id.clone(),
            expected_plan_hash: baseline.plan().plan_hash.clone(),
            expected_output_hash: Some(preview.output_hash.clone()),
        },
        &baseline,
    )
    .unwrap();
    assert_eq!(applied.output_hash, preview.output_hash);
    assert_eq!(applied.conversion.asset, baseline.candidate().asset);

    let mut provenance_request = baseline_request.clone();
    provenance_request.license_path = Some("licenses/replacement.txt".to_owned());
    let changed_provenance = plan_conversion(&provenance_request, &imported).unwrap();
    assert_ne!(changed_provenance.plan().plan_id, baseline.plan().plan_id);
    assert_ne!(
        changed_provenance.plan().plan_hash,
        baseline.plan().plan_hash
    );
    assert_ne!(
        changed_provenance.candidate().asset.content_hash,
        baseline.candidate().asset.content_hash
    );
    assert_eq!(
        changed_provenance.candidate().asset.voxel_data_hash,
        baseline.candidate().asset.voxel_data_hash
    );

    let mut transformed_request = baseline_request;
    transformed_request.settings.conversion.origin_policy =
        VoxelConversionOriginPolicy::SourceOrigin;
    transformed_request.settings.transform[12] = 10.0;
    let transformed = plan_conversion(&transformed_request, &imported).unwrap();
    assert_ne!(transformed.plan().plan_hash, baseline.plan().plan_hash);
    assert_ne!(
        transformed.candidate().asset.voxel_data_hash,
        baseline.candidate().asset.voxel_data_hash
    );

    let stale = preview_conversion(
        &ConversionPreviewRequest {
            plan_id: baseline.plan().plan_id.clone(),
            expected_plan_hash: hash('b'),
            max_samples: 2,
        },
        &baseline,
    )
    .unwrap_err();
    assert_eq!(stale.diagnostics()[0].code, "conversion.stalePlan");
}

#[test]
fn prepared_plan_provenance_cannot_be_rewritten_and_rehashed() {
    let imported = imported_source();
    let baseline = plan_conversion(&plan_request(&imported), &imported).unwrap();
    let mut forged_plan = baseline.plan().clone();
    forged_plan.license_path = Some("licenses/forged.txt".to_owned());
    forged_plan.plan_hash = conversion_plan_hash(&forged_plan);
    let forged = apply_conversion(
        &ConversionApplyRequest {
            plan_id: forged_plan.plan_id,
            expected_plan_hash: forged_plan.plan_hash,
            expected_output_hash: None,
        },
        &baseline,
    )
    .unwrap_err();
    assert_eq!(forged.diagnostics()[0].code, "conversion.stalePlan");
}

#[test]
fn cover_fit_fallback_and_texture_palette_are_authority_owned() {
    let imported = imported_source();
    let mut fallback_request = plan_request(&imported);
    fallback_request.settings.conversion.fit_policy = VoxelConversionFitPolicy::Cover;
    fallback_request.settings.conversion.material_map.pop();
    fallback_request
        .settings
        .material_policy
        .default_voxel_material = Some(8);
    let fallback = plan_conversion(&fallback_request, &imported).unwrap();
    assert!(fallback
        .candidate()
        .asset
        .material_map
        .iter()
        .any(|mapping| mapping.source_material_slot == 1 && mapping.voxel_material_slot == 8));

    let mut texture_request = plan_request(&imported);
    texture_request.settings.conversion.material_map.clear();
    let texture = texture_source(hash('c'));
    texture_request.settings.material_policy.texture_assets = vec![TextureSampleAsset {
        texture: texture.clone(),
        texel_materials: vec![7, 8],
    }];
    let uv_hash = imported.receipt.metadata.texture_coordinates[0]
        .source_hash
        .clone();
    texture_request.settings.material_policy.texture_bindings = vec![
        texture_binding(0, texture.clone(), uv_hash.clone(), [0.0, 0.0]),
        texture_binding(1, texture.clone(), uv_hash, [1.0, 0.0]),
    ];
    let textured = plan_conversion(&texture_request, &imported).unwrap();
    let resolved = textured
        .candidate()
        .asset
        .material_map
        .iter()
        .map(|mapping| (mapping.source_material_slot, mapping.voxel_material_slot))
        .collect::<Vec<_>>();
    assert_eq!(resolved, vec![(0, 7), (1, 8)]);

    texture_request.settings.material_policy.texture_bindings[0]
        .texture
        .content_hash = hash('d');
    let error = plan_conversion(&texture_request, &imported).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        "conversion.textureHashMismatch"
    );
}

#[test]
fn texture_palette_uses_hash_pinned_barycentric_uv_per_voxel() {
    let mut source_request = import_request();
    source_request.mesh_primitive = Some("group/0".to_owned());
    let mut imported = import_mesh_source(&source_request).unwrap();
    for (index, coordinate) in imported.mesh.texture_coordinates[0]
        .coordinates
        .iter_mut()
        .enumerate()
    {
        *coordinate = Some([f64::from((index % 4 >= 2) as u8), 0.0]);
    }
    imported.receipt.metadata.texture_coordinates[0].source_hash =
        texture_coordinate_source_hash(&imported.mesh, 0).unwrap();
    let mut request = plan_request(&imported);
    request.settings.conversion.material_map.clear();
    request.settings.conversion.resolution = [8, 8, 8];
    request.settings.conversion.max_output_voxels = 512;
    let texture = texture_source(hash('e'));
    request.settings.material_policy.texture_assets = vec![TextureSampleAsset {
        texture: texture.clone(),
        texel_materials: vec![7, 8],
    }];
    let uv_hash = imported.receipt.metadata.texture_coordinates[0]
        .source_hash
        .clone();
    request.settings.material_policy.texture_bindings =
        vec![texture_binding(0, texture, uv_hash, [0.0, 0.0])];

    let prepared = plan_conversion(&request, &imported).unwrap();
    let asset = &prepared.candidate().asset;
    let info = query_model_info(
        asset,
        &VoxelModelInfoRequest {
            expected_content_hash: asset.content_hash.clone(),
            include_material_counts: true,
        },
    )
    .unwrap();
    assert_eq!(
        info.material_counts
            .iter()
            .map(|count| count.material_slot)
            .collect::<Vec<_>>(),
        vec![7, 8]
    );

    let mut different_fallback = request.clone();
    different_fallback.settings.material_policy.texture_bindings[0].sample_uv = [1.0, 0.0];
    let fallback_changed = plan_conversion(&different_fallback, &imported).unwrap();
    assert_eq!(
        fallback_changed.candidate().asset.voxel_data_hash,
        prepared.candidate().asset.voxel_data_hash
    );

    let mut stale_uv = request;
    stale_uv.settings.material_policy.texture_bindings[0]
        .uv_attribute
        .source_hash = hash('f');
    let error = plan_conversion(&stale_uv, &imported).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.uvHashMismatch");
}

#[test]
fn solid_topology_limits_and_rejected_install_leave_no_partial_output() {
    let imported = imported_source();
    let mut solid_request = plan_request(&imported);
    solid_request.settings.conversion.mode = VoxelConversionMode::Solid;
    let error = plan_conversion(&solid_request, &imported).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        "conversion.unsupportedTopology"
    );

    let mut oversized = plan_request(&imported);
    oversized.settings.conversion.resolution[0] = MAX_CONVERSION_RESOLUTION_AXIS + 1;
    let error = plan_conversion(&oversized, &imported).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.resourceLimit");

    let prepared = plan_conversion(&plan_request(&imported), &imported).unwrap();
    let directory = temporary_directory();
    fs::create_dir(&directory).unwrap();
    let output = directory.join("known-good.voxel.json");
    fs::write(&output, "known-good\n").unwrap();
    let stale_apply = ConversionApplyRequest {
        plan_id: prepared.plan().plan_id.clone(),
        expected_plan_hash: prepared.plan().plan_hash.clone(),
        expected_output_hash: Some(hash('e')),
    };
    assert!(apply_conversion_and_install(&stale_apply, &prepared, &output).is_err());
    assert_eq!(fs::read_to_string(&output).unwrap(), "known-good\n");
    assert!(!directory.join("known-good.voxel.json.pending").exists());

    let valid_apply = ConversionApplyRequest {
        plan_id: prepared.plan().plan_id.clone(),
        expected_plan_hash: prepared.plan().plan_hash.clone(),
        expected_output_hash: Some(prepared.candidate().asset.content_hash.clone()),
    };
    let applied = apply_conversion_and_install(&valid_apply, &prepared, &output).unwrap();
    assert_eq!(
        decode_voxel_asset(&fs::read_to_string(&output).unwrap()).unwrap(),
        applied.conversion.asset
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn model_info_and_windows_are_bounded_deterministic_and_stale_safe() {
    let imported = imported_source();
    let prepared = plan_conversion(&plan_request(&imported), &imported).unwrap();
    let asset = &prepared.candidate().asset;
    let info = query_model_info(
        asset,
        &VoxelModelInfoRequest {
            expected_content_hash: asset.content_hash.clone(),
            include_material_counts: true,
        },
    )
    .unwrap();
    assert_eq!(info.voxel_count, prepared.candidate().output_voxels);
    assert_eq!(
        info.material_counts
            .iter()
            .map(|count| count.voxel_count)
            .sum::<usize>(),
        info.voxel_count
    );

    let window = query_model_window(
        asset,
        &VoxelModelWindowRequest {
            expected_content_hash: asset.content_hash.clone(),
            bounds: asset.bounds,
            include_empty: false,
            material_filter: vec![7],
            max_samples: 1,
        },
    )
    .unwrap();
    assert_eq!(window.samples.len(), 1);
    assert_eq!(window.samples[0].material_slot, Some(7));
    assert!(window.samples_truncated);

    let stale = query_model_info(
        asset,
        &VoxelModelInfoRequest {
            expected_content_hash: hash('f'),
            include_material_counts: false,
        },
    )
    .unwrap_err();
    assert_eq!(
        stale.diagnostics()[0].code,
        "conversion.staleAuthoritySnapshot"
    );

    let invalid = query_model_window(
        asset,
        &VoxelModelWindowRequest {
            expected_content_hash: asset.content_hash.clone(),
            bounds: VoxelAssetBounds {
                min: [1, 0, 0],
                max: [0, 0, 0],
            },
            include_empty: true,
            material_filter: Vec::new(),
            max_samples: 1,
        },
    )
    .unwrap_err();
    assert_eq!(
        invalid.diagnostics()[0].code,
        "conversion.invalidQueryBounds"
    );
}

fn import_request() -> MeshSourceImportRequest {
    MeshSourceImportRequest {
        source_asset_id: "mesh/kenney-wall-a".to_string(),
        asset_version: 1,
        source_path: "fixtures/voxel-conversion/kenney-wall-a.glb".to_string(),
        format: MeshSourceFormat::Glb,
        source_bytes: SOURCE.to_vec(),
        expected_source_sha256: Some(SOURCE_HASH.to_string()),
        mesh_primitive: None,
    }
}

fn imported_source() -> voxel_convert::ImportedMeshSource {
    import_mesh_source(&import_request()).unwrap()
}

fn plan_request(source: &voxel_convert::ImportedMeshSource) -> ConversionPlanRequest {
    let legacy = decode_conversion_request(REQUEST).unwrap();
    ConversionPlanRequest {
        source: source.receipt.source.clone(),
        target_asset_id: legacy.asset_id,
        license_path: legacy.license_path,
        settings: ConversionPlanSettings {
            conversion: legacy.settings,
            transform: identity_transform(),
            material_policy: ConversionMaterialPolicy::default(),
        },
    }
}

fn texture_source(content_hash: String) -> TextureSourceRef {
    TextureSourceRef {
        texture_asset_id: "texture/conversion-palette".to_string(),
        asset_version: 1,
        content_hash,
        width: 2,
        height: 1,
        color_space: TextureColorSpace::Linear,
        channel_layout: TextureChannelLayout::PaletteIndexU16,
    }
}

fn texture_binding(
    source_material_slot: u32,
    texture: TextureSourceRef,
    source_hash: String,
    sample_uv: [f64; 2],
) -> TextureMaterialBinding {
    TextureMaterialBinding {
        source_material_slot,
        texture,
        uv_attribute: TextureUvAttributeRef {
            attribute_name: "TEXCOORD_0".to_string(),
            source_hash,
        },
        sample_uv,
        sampling_policy: TextureSamplingPolicy::NearestTexel,
        wrap_policy: TextureWrapPolicy::ClampToEdge,
        material_mode: TextureMaterialMode::SamplePaletteIndex,
    }
}

fn glb_with_shared_primitive_material() -> Vec<u8> {
    const JSON_CHUNK_TYPE: u32 = 0x4e4f_534a;
    assert_eq!(&SOURCE[..4], b"glTF");
    let json_len = u32::from_le_bytes(SOURCE[12..16].try_into().unwrap()) as usize;
    assert_eq!(
        u32::from_le_bytes(SOURCE[16..20].try_into().unwrap()),
        JSON_CHUNK_TYPE
    );
    let json_end = 20 + json_len;
    let mut document: serde_json::Value = serde_json::from_slice(&SOURCE[20..json_end]).unwrap();
    let first_material = document["meshes"][0]["primitives"][0]["material"].clone();
    document["meshes"][0]["primitives"][1]["material"] = first_material;

    let mut json = serde_json::to_vec(&document).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total_len = 20 + json.len() + SOURCE.len() - json_end;
    let mut glb = Vec::with_capacity(total_len);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total_len as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(&JSON_CHUNK_TYPE.to_le_bytes());
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&SOURCE[json_end..]);
    glb
}

fn hash(digit: char) -> String {
    format!("sha256:{}", digit.to_string().repeat(64))
}

fn temporary_directory() -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "rusty-engine-conversion-authority-{}-{unique}",
        std::process::id()
    ))
}
