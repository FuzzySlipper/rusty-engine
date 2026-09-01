use std::fs;
use std::path::PathBuf;
use std::process::Command;

use asset_catalog::{decode_catalog, validate_catalog};
use asset_import::*;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use core_space::{WorldPos, WorldVec};
use render_model::{
    pack_mesh_resources, AnimatedMeshAsset, CollisionResolution, MeshPayloadSource,
    MeshResourceEncoding, MESH_RESOURCE_MAGIC_V2,
};
use sha2::Digest;
use svc_collision::{
    Ray, StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance,
    StaticMeshCollisionProjection, StaticMeshInstanceId, StaticMeshTransform,
};
use voxel_convert::{import_mesh_source, MeshSourceFormat, MeshSourceImportRequest};

const VALID: &str = r#"{
  "schemaVersion": 1,
  "name": "fixture-triangle",
  "positions": [0, 0, 0, 1, 0, 0, 0, 1, 0],
  "normals": [0, 0, 1, 0, 0, 1, 0, 0, 1],
  "indices": [0, 1, 2],
  "materials": [
    {"slot": 0, "name": "steel", "color": [0.5, 0.6, 0.7, 1], "texture": "steel-plate"}
  ],
  "groups": [{"materialSlot": 0, "start": 0, "count": 3}],
  "collision": "aabbFallback"
}"#;

const TEXTURED_VALID: &str = r#"{
  "schemaVersion": 1,
  "name": "textured-triangle",
  "positions": [0, 0, 0, 1, 0, 0, 0, 1, 0],
  "normals": [0, 0, 1, 0, 0, 1, 0, 0, 1],
  "uvs": [0, 0, 1, 0, 0, 1],
  "indices": [0, 1, 2],
  "materials": [
    {"slot": 0, "name": "checker", "color": [1, 1, 1, 1], "texture": "checker"}
  ],
  "groups": [{"materialSlot": 0, "start": 0, "count": 3}],
  "collision": "visualOnly"
}"#;

const ANIMATED_GLB: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
));

const LOADING_BAY_BUTTON_GLB: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/render/assets/kenney-factory-kit/button-floor-square.glb"
));

const STATIC_UNLIT_GLB_BASE64: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/render/assets/static-unlit-triangle.glb.base64"
));

const STATIC_RAMP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/collision/static-ramp.mesh.json"
));

fn uri() -> SourceUri {
    SourceUri::RelativePath("assets/fixture-triangle.mesh.json".to_owned())
}

fn animated_asset_stem(source: &[u8]) -> String {
    format!("{:x}", sha2::Sha256::digest(source))
}

#[test]
fn valid_source_produces_deterministic_native_assets_and_manifest() {
    let context = ImportContext::with_textures(["steel-plate".to_owned()]);
    let first = plan_import(&uri(), VALID, &context, ImportMode::DryRun, None, None);
    let second = plan_import(&uri(), VALID, &context, ImportMode::DryRun, None, None);
    assert_eq!(first.files, second.files);
    assert_eq!(first.manifest, second.manifest);
    assert!(!first.has_errors);
    assert!(first.report.contains("dry-run leaves storage unchanged"));

    let imported = import_text(VALID, uri().value(), &context);
    let assets = imported.assets.unwrap();
    assets.static_mesh.validate().unwrap();
    assert!(validate_catalog(&assets.catalog).is_ok());
    assert_eq!(
        assets.static_mesh.payload.provenance,
        render_model::MeshProvenance::StaticAsset
    );
}

#[test]
fn authored_uvs_reach_static_mesh_and_partition_into_packed_v2_bytes() {
    let imported = import_text(
        TEXTURED_VALID,
        "textured-triangle.mesh.json",
        &ImportContext::with_textures(["checker".to_owned()]),
    );
    assert!(!imported.has_errors(), "{:?}", imported.diagnostics);
    let mesh = imported.assets.unwrap().static_mesh;
    assert!(mesh
        .payload
        .layout
        .attributes
        .iter()
        .any(|attribute| attribute.name == render_model::MeshAttributeName::Uv));
    assert!(matches!(
        &mesh.payload.source,
        MeshPayloadSource::Inline { uvs: Some(uvs), .. }
            if uvs == &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]
    ));

    let packed = pack_mesh_resources(&[mesh.payload], 1024).unwrap();
    assert_eq!(packed.payloads.len(), 1);
    assert_eq!(packed.resources.len(), 1);
    assert_eq!(&packed.resources[0].bytes[..8], &MESH_RESOURCE_MAGIC_V2);
    assert!(matches!(
        packed.payloads[0].source,
        MeshPayloadSource::Resource {
            encoding: MeshResourceEncoding::PackedStreamsLeV2,
            uvs_byte_offset: Some(_),
            ..
        }
    ));
}

#[test]
fn authored_uvs_are_optional_but_must_match_vertices_and_be_finite() {
    let legacy = import_text(VALID, "legacy.mesh.json", &ImportContext::default());
    assert!(!legacy.has_errors(), "{:?}", legacy.diagnostics);
    assert!(matches!(
        legacy.assets.unwrap().static_mesh.payload.source,
        MeshPayloadSource::Inline { uvs: None, .. }
    ));

    let mismatched = TEXTURED_VALID.replace("\"uvs\": [0, 0, 1, 0, 0, 1]", "\"uvs\": [0, 0, 1, 0]");
    let rejected = import_text(
        &mismatched,
        "mismatched-uv.mesh.json",
        &ImportContext::default(),
    );
    assert!(rejected.assets.is_none());
    assert!(rejected
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ImportCode::AttributeLengthMismatch));

    let mut parsed = parse_source(TEXTURED_VALID, "non-finite-uv.mesh.json")
        .mesh
        .unwrap();
    parsed.uvs.as_mut().unwrap()[2] = f32::INFINITY;
    let rejected = import(&parsed);
    assert!(rejected.assets.is_none());
    assert!(rejected
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ImportCode::NonFiniteValue));
}

#[test]
fn golden_static_ramp_imports_into_the_shared_trimesh_query_service() {
    let imported = import_text(
        STATIC_RAMP,
        "fixtures/collision/static-ramp.mesh.json",
        &ImportContext::default(),
    );
    assert!(!imported.has_errors(), "{:?}", imported.diagnostics);
    let mesh = imported.assets.unwrap().static_mesh;
    let CollisionResolution::Trimesh { payload } = mesh.resolve_collision() else {
        panic!("fixture must resolve exact triangle collision");
    };
    let MeshPayloadSource::Inline {
        positions, indices, ..
    } = payload.source
    else {
        panic!("offline import collision geometry is resolved before renderer packing");
    };
    let asset = StaticMeshColliderAsset::new(
        StaticMeshAssetId(1),
        positions
            .as_chunks::<3>()
            .0
            .iter()
            .map(|point| [point[0] as f64, point[1] as f64, point[2] as f64])
            .collect(),
        indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|triangle| [triangle[0], triangle[1], triangle[2]])
            .collect(),
    )
    .unwrap();
    let hash = asset.geometry_hash;
    let mut collision = StaticMeshCollisionProjection::default();
    collision
        .replace_all(
            0,
            [asset],
            [StaticMeshColliderInstance {
                id: StaticMeshInstanceId(2),
                asset: StaticMeshAssetId(1),
                expected_geometry_hash: hash,
                transform: StaticMeshTransform::IDENTITY,
            }],
        )
        .unwrap();
    let ramp = collision
        .raycast(
            Ray::new(WorldPos::new(1.0, 3.0, 0.0), WorldVec::new(0.0, -1.0, 0.0)),
            10.0,
        )
        .unwrap();
    assert!((ramp.point.y - 1.0).abs() < 1.0e-6);
    assert!(collision.swept_aabb_overlaps(
        WorldPos::new(2.0, 0.5, -0.25),
        WorldPos::new(2.5, 1.5, 0.25),
        WorldVec::new(1.0, 0.0, 0.0),
    ));
}

#[test]
fn animated_glb_produces_deterministic_runtime_resource_descriptor_and_provenance() {
    let uri = SourceUri::RelativePath("content/actors/character-medium.glb".to_owned());
    let asset_stem = animated_asset_stem(ANIMATED_GLB);
    let first = plan_animated_glb_import(
        &uri,
        ANIMATED_GLB,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    let second = plan_animated_glb_import(
        &uri,
        ANIMATED_GLB,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(!first.has_errors);
    assert_eq!(first.files, second.files);
    assert_eq!(first.manifest, second.manifest);
    assert!(matches!(
        first.reimport,
        Some(ReimportPlan::StructuralReload { .. })
    ));

    let resource = artifact(&first, "character-medium.glb");
    assert_eq!(resource.bytes, ANIMATED_GLB);
    let descriptor: AnimatedMeshAsset = serde_json::from_slice(
        &artifact(&first, &format!("{asset_stem}.animated-mesh.json")).bytes,
    )
    .unwrap();
    descriptor.validate().unwrap();
    assert_eq!(descriptor.asset, format!("mesh-animation/{asset_stem}"));
    assert_eq!(
        descriptor.content_hash.as_deref(),
        Some("sha256:c71255a41c0373f0d2ef52593369d5fd9d2f6220ae548aff8cd6bf5edb403674")
    );
    assert_eq!(
        descriptor
            .clips
            .iter()
            .map(|clip| clip.id.as_str())
            .collect::<Vec<_>>(),
        ["idle", "run", "jump"]
    );
    assert_eq!(descriptor.default_clip.as_deref(), Some("idle"));
    assert_eq!(
        descriptor.embedded_material_slots,
        vec![render_model::AnimatedMeshEmbeddedMaterialSlot {
            slot: 0,
            source_material_slot: 0,
        }]
    );
    assert!(descriptor.material_slots.is_empty());

    let catalog = decode_catalog(
        std::str::from_utf8(&artifact(&first, &format!("{asset_stem}.catalog.json")).bytes)
            .unwrap(),
    )
    .unwrap();
    assert!(validate_catalog(&catalog).is_ok());
    let entry = catalog.entries.first().unwrap();
    assert_eq!(entry.id.as_str(), descriptor.asset);
    assert_eq!(entry.source_path.as_deref(), Some("character-medium.glb"));

    let manifest = first.manifest.as_ref().unwrap();
    assert_eq!(
        manifest.source_schema_version,
        SUPPORTED_ANIMATED_GLB_VERSION
    );
    assert_eq!(manifest.importer_version, IMPORTER_VERSION);
    assert_eq!(manifest.mesh_asset_id, descriptor.asset);
    assert_eq!(
        manifest.source_hash,
        "c71255a41c0373f0d2ef52593369d5fd9d2f6220ae548aff8cd6bf5edb403674"
    );

    let imported = import_animated_glb_asset(&uri, ANIMATED_GLB, &ImportContext::default())
        .assets
        .unwrap();
    assert_eq!(imported.receipt.node_count, 61);
    assert_eq!(imported.receipt.skin_count, 1);
    assert_eq!(imported.receipt.joint_count, 45);
    assert_eq!(imported.receipt.material_count, 1);
    assert_eq!(imported.receipt.texture_count, 1);
    assert_eq!(imported.receipt.image_count, 1);
    assert_eq!(imported.receipt.animation_kind, GlbAnimationKind::Animated);
    assert_eq!(imported.receipt.clip_count, 3);
    assert_eq!(imported.receipt.channel_count, 56);
    assert_eq!(imported.receipt.keyframe_count, 1048);
    let rig = descriptor
        .rig
        .as_ref()
        .expect("multi-root animated GLB retains an importer-derived rig");
    assert!(rig.structural_root_ids.len() > 1);
    assert!(rig.designated_motion_root_ids.is_empty());
    assert!(!rig.authored_pose_translation_joint_ids.is_empty());
}

#[test]
fn animated_glb_filename_spelling_is_provenance_only_for_identity() {
    let expected_asset = format!("mesh-animation/{}", animated_asset_stem(ANIMATED_GLB));
    for source_path in [
        "content/actors/UAL1_Standard.glb",
        "content/actors/UAL1 Standard.glb",
        "content/actors/ual1-standard.glb",
    ] {
        let imported = import_animated_glb_asset(
            &SourceUri::RelativePath(source_path.to_owned()),
            ANIMATED_GLB,
            &ImportContext::default(),
        );
        assert!(
            !imported.has_errors(),
            "{source_path}: {:?}",
            imported.diagnostics
        );
        let assets = imported.assets.expect("filename spelling is not identity");
        assert_eq!(assets.animated_mesh.asset, expected_asset);
        assert_eq!(assets.runtime_resource_bytes, ANIMATED_GLB);
        assert_eq!(
            assets.catalog.entries[0].id.as_str(),
            expected_asset.as_str()
        );
    }
}

#[test]
fn animated_glb_invalid_content_still_rejects_after_filename_admission() {
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath("content/actors/Not A Real GLB.glb".to_owned()),
        b"not a GLB",
        &ImportContext::default(),
    );
    assert!(imported.assets.is_none());
    assert!(imported.has_errors());
    assert!(imported
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ImportCode::InvalidContainer));
}

#[test]
fn animated_glb_admits_bounded_texture_transform_and_retains_exact_bytes() {
    let source = rewrite_glb_json(ANIMATED_GLB, |root| {
        root["extensionsUsed"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::Value::String(
                "KHR_texture_transform".to_owned(),
            ));
        root["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["extensions"] = serde_json::json!({
            "KHR_texture_transform": {
                "offset": [-0.25, 0.5],
                "rotation": 0.75,
                "scale": [2.0, -3.0],
                "texCoord": 0
            }
        });
    });
    let uri = SourceUri::RelativePath("content/actors/transformed-character.glb".to_owned());
    let imported = import_animated_glb_asset(&uri, &source, &ImportContext::default());
    assert!(!imported.has_errors(), "{:?}", imported.diagnostics);
    let assets = imported.assets.expect("bounded transform is admitted");
    assert_eq!(assets.runtime_resource_bytes, source);
    assert_eq!(
        assets.receipt.source_hash,
        format!("sha256:{:x}", sha2::Sha256::digest(&source))
    );
    assert_eq!(assets.receipt.clip_count, 3);
}

#[test]
fn animated_glb_rejects_malformed_or_unrealizable_texture_transforms_atomically() {
    let cases = [
        (
            serde_json::json!({"offset": [0.0]}),
            ImportCode::InvalidContainer,
        ),
        (
            serde_json::json!({"rotation": "quarter-turn"}),
            ImportCode::InvalidContainer,
        ),
        (
            serde_json::json!({"scale": [1_000_001.0, 1.0]}),
            ImportCode::ResourceLimit,
        ),
        (
            serde_json::json!({"texCoord": 4}),
            ImportCode::UnsupportedFeature,
        ),
        (
            serde_json::json!({"center": [0.5, 0.5]}),
            ImportCode::UnsupportedFeature,
        ),
    ];
    for (transform, expected_code) in cases {
        let source = rewrite_glb_json(ANIMATED_GLB, |root| {
            root["extensionsUsed"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::Value::String(
                    "KHR_texture_transform".to_owned(),
                ));
            root["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["extensions"] =
                serde_json::json!({"KHR_texture_transform": transform});
        });
        let plan = plan_animated_glb_import(
            &SourceUri::RelativePath("content/actors/invalid-transform.glb".to_owned()),
            &source,
            &ImportContext::default(),
            ImportMode::DryRun,
            None,
            None,
        );
        assert!(plan.has_errors);
        assert!(plan.files.is_empty());
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == expected_code),
            "{:?}",
            plan.diagnostics
        );
    }
}

#[test]
fn exact_loading_bay_fixture_reaches_the_independent_external_image_boundary() {
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(LOADING_BAY_BUTTON_GLB)),
        "f32def1dd9a57939b096d64361fc5058a8ba240a0394951e8681fb7326ebdeb6"
    );
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath("content/loading-bay/button-floor-square.glb".to_owned()),
        LOADING_BAY_BUTTON_GLB,
        &ImportContext::default(),
    );
    assert!(imported.assets.is_none());
    assert_eq!(imported.diagnostics.len(), 1, "{:?}", imported.diagnostics);
    assert_eq!(imported.diagnostics[0].code, ImportCode::ExternalResource);
    assert_eq!(imported.diagnostics[0].locus, "source.images[0]");
}

#[test]
fn binary_glb_closure_packs_external_image_and_admits_embedded_clips() {
    let png = BASE64
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL95wAAAABJRU5ErkJggg==")
        .unwrap();
    let external_character = rewrite_glb_json(ANIMATED_GLB, |root| {
        let image = root["images"][0].as_object_mut().unwrap();
        image.remove("bufferView");
        image.remove("mimeType");
        image.insert(
            "uri".to_owned(),
            serde_json::Value::String("Textures/character.png".to_owned()),
        );
    });
    let closure = GlbSourceClosure {
        root_glb: external_character.clone(),
        resources: vec![GltfResource {
            uri: "Textures/character.png".to_owned(),
            bytes: png,
        }],
    };
    assert_eq!(
        glb_relative_resource_uris(&external_character).unwrap(),
        ["Textures/character.png"]
    );
    let first = admit_glb_source(&closure).unwrap();
    let second = admit_glb_source(&closure).unwrap();
    assert_eq!(first, second);
    assert_ne!(first.glb_bytes, external_character);
    assert!(glb_relative_resource_uris(&first.glb_bytes)
        .unwrap()
        .is_empty());
    assert_eq!(first.external_resource_uris, ["Textures/character.png"]);
    assert_eq!(
        first.source_byte_count,
        (external_character.len() + closure.resources[0].bytes.len()) as u64
    );

    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath("content/actors/character.glb".to_owned()),
        &first.glb_bytes,
        &ImportContext::default(),
    );
    assert!(!imported.has_errors(), "{:?}", imported.diagnostics);
    let descriptor = imported.assets.unwrap().animated_mesh;
    let clips = descriptor
        .clips
        .iter()
        .map(|clip| clip.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(clips, ["idle", "run", "jump"]);
}

#[test]
fn exact_loading_bay_closure_admits_visual_metadata_and_embedded_clips() {
    let png = BASE64
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL95wAAAABJRU5ErkJggg==")
        .unwrap();
    let packed = admit_glb_source(&GlbSourceClosure {
        root_glb: LOADING_BAY_BUTTON_GLB.to_vec(),
        resources: vec![GltfResource {
            uri: "Textures/colormap.png".to_owned(),
            bytes: png,
        }],
    })
    .unwrap();
    assert!(glb_relative_resource_uris(&packed.glb_bytes)
        .unwrap()
        .is_empty());
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath("content/loading-bay/button-floor-square.glb".to_owned()),
        &packed.glb_bytes,
        &ImportContext::default(),
    );
    assert!(!imported.has_errors(), "{:?}", imported.diagnostics);
    let assets = imported.assets.unwrap();
    assert_eq!(assets.runtime_resource_bytes, packed.glb_bytes);
    assets.animated_mesh.validate().unwrap();
    assert_eq!(
        assets
            .animated_mesh
            .clips
            .iter()
            .map(|clip| clip.id.as_str())
            .collect::<Vec<_>>(),
        ["toggle-on", "toggle-off", "toggle"]
    );
    assert!(!assets.animated_mesh.embedded_material_slots.is_empty());
    assert!(assets
        .animated_mesh
        .bounds
        .min
        .iter()
        .chain(assets.animated_mesh.bounds.max.iter())
        .all(|value| value.is_finite()));
}

#[test]
fn zero_clip_unlit_glb_uses_the_existing_mesh_resource_lifecycle() {
    let uri = SourceUri::RelativePath("content/environment/static-unlit-triangle.glb".to_owned());
    let source = static_unlit_glb();
    let asset_stem = animated_asset_stem(&source);
    let first = plan_animated_glb_import(
        &uri,
        &source,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    let second = plan_animated_glb_import(
        &uri,
        &source,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(!first.has_errors, "{:?}", first.diagnostics);
    assert_eq!(first.files, second.files);
    assert_eq!(first.manifest, second.manifest);
    assert!(first.report.contains("kind: staticGlb"));

    let resource = artifact(&first, "static-unlit-triangle.glb");
    assert_eq!(resource.bytes, source);
    let descriptor: AnimatedMeshAsset = serde_json::from_slice(
        &artifact(&first, &format!("{asset_stem}.animated-mesh.json")).bytes,
    )
    .unwrap();
    descriptor.validate().unwrap();
    assert_eq!(descriptor.asset, format!("mesh-animation/{asset_stem}"));
    assert!(descriptor.clips.is_empty());
    assert_eq!(descriptor.default_clip, None);
    assert_eq!(
        descriptor.embedded_material_slots,
        vec![render_model::AnimatedMeshEmbeddedMaterialSlot {
            slot: 0,
            source_material_slot: 0,
        }]
    );
    assert!(descriptor.material_slots.is_empty());

    let imported = import_animated_glb_asset(&uri, &source, &ImportContext::default())
        .assets
        .unwrap();
    assert_eq!(imported.receipt.animation_kind, GlbAnimationKind::Static);
    assert_eq!(imported.receipt.clip_count, 0);
    assert_eq!(imported.receipt.channel_count, 0);
    assert_eq!(imported.receipt.keyframe_count, 0);
    assert_eq!(imported.receipt.material_count, 1);
    assert_eq!(imported.receipt.primitive_count, 1);
}

#[test]
fn gltf_closure_converges_with_glb_for_static_and_animated_sources() {
    let static_glb = static_triangle_glb();
    let static_source = external_gltf(&static_glb, "geometry.bin", None);
    let packed = admit_gltf_source(&static_source).unwrap();
    let static_import = import_mesh_source(&MeshSourceImportRequest {
        source_asset_id: "mesh/static-triangle".to_owned(),
        asset_version: 1,
        source_path: "static-triangle.gltf".to_owned(),
        format: MeshSourceFormat::Glb,
        source_bytes: packed.glb_bytes,
        expected_source_sha256: None,
        mesh_primitive: None,
    })
    .unwrap();
    assert_eq!(static_import.mesh.positions.len(), 3);
    assert_eq!(static_import.mesh.triangles[0].indices, [0, 1, 2]);

    let animated_source = external_gltf(ANIMATED_GLB, "actor.bin", Some("textures/actor.png"));
    let uri = SourceUri::RelativePath("content/actors/actor-external.gltf".to_owned());
    let first = plan_animated_gltf_import(
        &uri,
        &animated_source,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    let second = plan_animated_gltf_import(
        &uri,
        &animated_source,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(!first.has_errors, "{:?}", first.diagnostics);
    assert_eq!(first.files, second.files);
    assert_eq!(first.manifest, second.manifest);
    assert!(artifact(&first, "actor-external.glb")
        .bytes
        .starts_with(b"glTF"));
    let asset_stem = animated_asset_stem(&artifact(&first, "actor-external.glb").bytes);
    let descriptor: AnimatedMeshAsset = serde_json::from_slice(
        &artifact(&first, &format!("{asset_stem}.animated-mesh.json")).bytes,
    )
    .unwrap();
    assert_eq!(descriptor.clips.len(), 3);
    assert_eq!(first.manifest.as_ref().unwrap().source_uri, uri.value());
}

#[test]
fn gltf_data_uris_and_resource_fingerprint_are_bounded_and_deterministic() {
    let mut source = external_gltf(ANIMATED_GLB, "actor.bin", Some("actor.png"));
    let buffer = source.resources.remove(0);
    let mut root: serde_json::Value = serde_json::from_slice(&source.root_json).unwrap();
    root["buffers"][0]["uri"] = serde_json::Value::String(format!(
        "data:application/octet-stream;base64,{}",
        BASE64.encode(&buffer.bytes)
    ));
    let image = source.resources.remove(0);
    root["images"][0]["uri"] = serde_json::Value::String(format!(
        "data:image/png;base64,{}",
        BASE64.encode(&image.bytes)
    ));
    source.root_json = serde_json::to_vec(&root).unwrap();
    assert!(gltf_relative_resource_uris(&source.root_json)
        .unwrap()
        .is_empty());
    let packed = admit_gltf_source(&source).unwrap();
    assert!(packed.external_resource_uris.is_empty());

    let external = external_gltf(ANIMATED_GLB, "actor.bin", None);
    let first = admit_gltf_source(&external).unwrap();
    let mut changed = external.clone();
    changed.resources[0].bytes[0] ^= 1;
    let second = admit_gltf_source(&changed).unwrap();
    assert_ne!(first.source_hash, second.source_hash);
}

#[test]
fn gltf_closure_rejects_ambient_paths_collisions_missing_and_unsupported_resources() {
    let base = external_gltf(&static_triangle_glb(), "geometry.bin", None);
    for uri in [
        "https://example.invalid/geometry.bin",
        "/tmp/geometry.bin",
        "../geometry.bin",
        "nested\\geometry.bin",
        "geometry.bin?revision=1",
    ] {
        let mut root: serde_json::Value = serde_json::from_slice(&base.root_json).unwrap();
        root["buffers"][0]["uri"] = serde_json::Value::String(uri.to_owned());
        let failure = gltf_relative_resource_uris(&serde_json::to_vec(&root).unwrap()).unwrap_err();
        assert_eq!(failure.code, ImportCode::ExternalResource, "{uri}");
    }

    let missing = GltfSourceClosure {
        root_json: base.root_json.clone(),
        resources: Vec::new(),
    };
    assert_eq!(
        admit_gltf_source(&missing).unwrap_err().code,
        ImportCode::ExternalResource
    );

    let mut duplicate = base.clone();
    duplicate.resources.push(duplicate.resources[0].clone());
    assert_eq!(
        admit_gltf_source(&duplicate).unwrap_err().code,
        ImportCode::MalformedSource
    );

    let mut wrong_length = base.clone();
    let mut wrong_root: serde_json::Value =
        serde_json::from_slice(&wrong_length.root_json).unwrap();
    wrong_root["buffers"][0]["byteLength"] = serde_json::Value::from(1);
    wrong_length.root_json = serde_json::to_vec(&wrong_root).unwrap();
    assert_eq!(
        admit_gltf_source(&wrong_length).unwrap_err().code,
        ImportCode::InvalidContainer
    );

    let mut unsupported_root: serde_json::Value = serde_json::from_slice(&base.root_json).unwrap();
    unsupported_root["images"] = serde_json::json!([{"uri":"texture.gif"}]);
    let unsupported = GltfSourceClosure {
        root_json: serde_json::to_vec(&unsupported_root).unwrap(),
        resources: vec![
            base.resources[0].clone(),
            GltfResource {
                uri: "texture.gif".to_owned(),
                bytes: vec![1, 2, 3],
            },
        ],
    };
    assert_eq!(
        admit_gltf_source(&unsupported).unwrap_err().code,
        ImportCode::UnsupportedFeature
    );

    let mut collision_root: serde_json::Value = serde_json::from_slice(&base.root_json).unwrap();
    collision_root["images"] = serde_json::json!([{"uri":"geometry%2Ebin"}]);
    let failure =
        gltf_relative_resource_uris(&serde_json::to_vec(&collision_root).unwrap()).unwrap_err();
    assert_eq!(failure.code, ImportCode::MalformedSource);

    let too_many = GltfSourceClosure {
        root_json: base.root_json,
        resources: (0..=MAX_GLTF_RESOURCE_COUNT)
            .map(|index| GltfResource {
                uri: format!("resource-{index}.bin"),
                bytes: vec![1],
            })
            .collect(),
    };
    assert_eq!(
        admit_gltf_source(&too_many).unwrap_err().code,
        ImportCode::ResourceLimit
    );
}

#[test]
fn animated_glb_reimport_and_settings_are_closed_and_structural() {
    let uri = SourceUri::RelativePath("content/actors/character-medium.glb".to_owned());
    let prior = plan_animated_glb_import(
        &uri,
        ANIMATED_GLB,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    )
    .manifest
    .unwrap();
    assert_eq!(plan_reimport(&prior, &prior), ReimportPlan::Noop);

    let mut changed = ANIMATED_GLB.to_vec();
    let last = changed.last_mut().expect("fixture is non-empty");
    *last ^= 1;
    let changed_plan = plan_animated_glb_import(
        &uri,
        &changed,
        &ImportContext::default(),
        ImportMode::DryRun,
        Some(&prior),
        None,
    );
    assert!(!changed_plan.has_errors, "{:?}", changed_plan.diagnostics);
    assert!(matches!(
        changed_plan.reimport,
        Some(ReimportPlan::StructuralReload { .. })
    ));

    for settings in [
        ImportSettings {
            scale: 2.0,
            ..ImportSettings::default()
        },
        ImportSettings {
            generate_collision: true,
            ..ImportSettings::default()
        },
        ImportSettings {
            material_namespace: Some("actors".to_owned()),
            ..ImportSettings::default()
        },
    ] {
        let plan = plan_animated_glb_import(
            &uri,
            ANIMATED_GLB,
            &ImportContext {
                available_textures: None,
                settings,
            },
            ImportMode::DryRun,
            Some(&prior),
            None,
        );
        assert!(plan.has_errors);
        assert!(plan.files.is_empty());
        assert!(plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == ImportCode::InvalidImportSettings));
    }
}

#[test]
fn animated_glb_rejects_external_over_quota_and_non_finite_sources_without_artifacts() {
    let external = test_glb(
        r#"{"asset":{"version":"2.0"},"buffers":[{"byteLength":4}],"images":[{"uri":"actor.png"}]}"#,
        &[0; 4],
    );
    let external_plan = plan_animated_glb_import(
        &SourceUri::RelativePath("content/actors/external.glb".to_owned()),
        &external,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(external_plan.has_errors);
    assert!(external_plan.files.is_empty());
    assert!(external_plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ImportCode::ExternalResource));

    let external_buffer = test_glb(
        r#"{
          "asset":{"version":"2.0"},
          "buffers":[
            {"byteLength":4},
            {"uri":"actor.bin","byteLength":4}
          ]
        }"#,
        &[0; 4],
    );
    let external_buffer_plan = plan_animated_glb_import(
        &SourceUri::RelativePath("content/actors/external-buffer.glb".to_owned()),
        &external_buffer,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(external_buffer_plan.has_errors);
    assert!(external_buffer_plan.files.is_empty());
    assert!(external_buffer_plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ImportCode::ExternalResource));

    let materials = std::iter::repeat_n("{}", MAX_ANIMATED_GLB_MATERIALS + 1)
        .collect::<Vec<_>>()
        .join(",");
    let over_quota = test_glb(
        &format!(
            "{{\"asset\":{{\"version\":\"2.0\"}},\"buffers\":[{{\"byteLength\":4}}],\"materials\":[{materials}]}}"
        ),
        &[0; 4],
    );
    let over_quota_plan = plan_animated_glb_import(
        &SourceUri::RelativePath("content/actors/too-many-materials.glb".to_owned()),
        &over_quota,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(over_quota_plan.has_errors);
    assert!(over_quota_plan.files.is_empty());
    assert!(over_quota_plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ImportCode::ResourceLimit));

    let malformed_plan = plan_animated_glb_import(
        &SourceUri::RelativePath("content/actors/malformed.glb".to_owned()),
        b"not a GLB",
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(malformed_plan.has_errors);
    assert!(malformed_plan.files.is_empty());
    assert!(malformed_plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ImportCode::InvalidContainer));

    let required_extension = test_glb(
        r#"{
          "asset":{"version":"2.0"},
          "extensionsUsed":["EXT_meshopt_compression"],
          "buffers":[{"byteLength":4}]
        }"#,
        &[0; 4],
    );
    let extension_plan = plan_animated_glb_import(
        &SourceUri::RelativePath("content/actors/compressed.glb".to_owned()),
        &required_extension,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(extension_plan.has_errors);
    assert!(extension_plan.files.is_empty());
    assert!(extension_plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ImportCode::UnsupportedFeature));

    let mut non_finite = static_unlit_glb();
    let bin = non_finite
        .windows(4)
        .rposition(|window| window == b"BIN\0")
        .expect("fixture contains a BIN chunk");
    non_finite[bin + 4..bin + 8].copy_from_slice(&f32::INFINITY.to_le_bytes());
    let non_finite_plan = plan_animated_glb_import(
        &SourceUri::RelativePath("content/actors/non-finite-static.glb".to_owned()),
        &non_finite,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(non_finite_plan.has_errors);
    assert!(non_finite_plan.files.is_empty());
    assert!(
        non_finite_plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == ImportCode::NonFiniteValue),
        "{:?}",
        non_finite_plan.diagnostics
    );

    let mut duplicate_clip = ANIMATED_GLB.to_vec();
    let jump = duplicate_clip
        .windows(4)
        .position(|window| window == b"jump")
        .expect("fixture contains jump clip");
    duplicate_clip[jump..jump + 4].copy_from_slice(b"idle");
    let duplicate_clip_plan = plan_animated_glb_import(
        &SourceUri::RelativePath("content/actors/duplicate-clip.glb".to_owned()),
        &duplicate_clip,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(duplicate_clip_plan.has_errors);
    assert!(duplicate_clip_plan.files.is_empty());
    assert!(
        duplicate_clip_plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == ImportCode::InvalidAnimation),
        "{:?}",
        duplicate_clip_plan.diagnostics
    );
}

#[test]
fn strict_source_and_topology_fail_without_artifacts() {
    let unsupported = VALID.replace(
        "  \"collision\": \"aabbFallback\"",
        "  \"animations\": [],\n  \"collision\": \"aabbFallback\"",
    );
    let plan = plan_import(
        &uri(),
        &unsupported,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(plan.has_errors);
    assert!(plan.files.is_empty());
    assert!(plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ImportCode::UnsupportedFeature));

    let bad_topology = VALID.replace("\"indices\": [0, 1, 2]", "\"indices\": [0, 1]");
    let outcome = import_text(&bad_topology, "bad.mesh.json", &ImportContext::default());
    assert!(outcome.assets.is_none());
    assert!(outcome
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ImportCode::UnsupportedTopology));
}

#[test]
fn sidecar_guid_provenance_reconcile_and_override_are_typed() {
    let metadata = init_metadata(
        uri(),
        VALID.as_bytes(),
        "mesh",
        IMPORTER_VERSION,
        ImportSettings::default(),
        "fixture-salt",
    );
    let encoded = encode_sidecar(&metadata).unwrap();
    assert_eq!(decode_sidecar(&encoded).unwrap(), metadata);
    assert!(
        decode_sidecar(&encoded.replace("\"schemaVersion\": 1", "\"schemaVersion\": 2")).is_err()
    );
    assert_eq!(
        reconcile(Some(&metadata), &uri(), VALID.as_bytes()),
        SidecarStatus::Unchanged
    );
    assert!(matches!(
        reconcile(
            Some(&metadata),
            &SourceUri::RelativePath("moved.mesh.json".to_owned()),
            VALID.as_bytes()
        ),
        SidecarStatus::MovedFile { .. }
    ));
    assert!(matches!(
        reconcile(Some(&metadata), &uri(), b"changed"),
        SidecarStatus::ContentChanged { .. }
    ));
    assert_eq!(
        detect_duplicate_guids(&[metadata.clone(), metadata.clone()]),
        vec![metadata.guid.clone()]
    );

    let base = metadata.import_settings.clone();
    let override_settings = ProjectOverride {
        guid: Some(metadata.guid.clone()),
        scale: Some(2.0),
        generate_collision: Some(true),
        material_namespace: Some(Some("factory".to_owned())),
    };
    let effective = override_settings.apply(&metadata.guid, &base).unwrap();
    assert_eq!(effective.scale, 2.0);
    assert_eq!(
        base, metadata.import_settings,
        "shared sidecar remains unchanged"
    );
}

#[test]
fn reimport_distinguishes_visual_and_structural_changes() {
    let context = ImportContext::default();
    let prior = plan_import(&uri(), VALID, &context, ImportMode::DryRun, None, None)
        .manifest
        .unwrap();
    assert_eq!(plan_reimport(&prior, &prior), ReimportPlan::Noop);

    let recolored = VALID.replace("0.5, 0.6, 0.7", "0.2, 0.3, 0.4");
    let visual = plan_import(
        &uri(),
        &recolored,
        &context,
        ImportMode::DryRun,
        Some(&prior),
        None,
    );
    assert!(matches!(
        visual.reimport,
        Some(ReimportPlan::VisualUpdate { .. })
    ));

    let reshaped = VALID.replace("1, 0, 0, 0, 1, 0", "2, 0, 0, 0, 1, 0");
    let structural = plan_import(
        &uri(),
        &reshaped,
        &context,
        ImportMode::DryRun,
        Some(&prior),
        None,
    );
    assert!(matches!(
        structural.reimport,
        Some(ReimportPlan::StructuralReload { .. })
    ));
}

#[test]
fn directory_publication_is_whole_and_failed_verification_preserves_prior() {
    let root = temp_directory("publication");
    let output = root.join("imported");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("prior.txt"), b"prior").unwrap();
    let mut plan = plan_import(
        &uri(),
        VALID,
        &ImportContext::default(),
        ImportMode::Write,
        None,
        None,
    );
    let receipt = publish_directory_atomically(&plan, &output).unwrap();
    assert!(receipt.replaced_previous);
    assert!(!output.join("prior.txt").exists());
    assert!(receipt
        .written_files
        .iter()
        .all(|path| output.join(path).is_file()));

    fs::write(output.join("sentinel.txt"), b"keep-me").unwrap();
    plan.files
        .iter_mut()
        .find(|file| file.relative_path.ends_with(".static-mesh.json"))
        .unwrap()
        .bytes = b"corrupt candidate".to_vec();
    assert!(publish_directory_atomically(&plan, &output).is_err());
    assert_eq!(fs::read(output.join("sentinel.txt")).unwrap(), b"keep-me");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sidecar_and_output_publication_roll_back_as_one_transaction() {
    let root = temp_directory("sidecar-transaction");
    let output = root.join("imported");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("prior.txt"), b"prior-output").unwrap();
    let sidecar = root.join("source.meta");
    fs::write(&sidecar, b"prior-sidecar").unwrap();

    let dry_run = plan_import(
        &uri(),
        VALID,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(matches!(
        publish_directory_with_sidecar_atomically(&dry_run, &output, &sidecar, b"next-sidecar"),
        Err(PublicationError::DryRun)
    ));
    assert_eq!(fs::read(output.join("prior.txt")).unwrap(), b"prior-output");
    assert_eq!(fs::read(&sidecar).unwrap(), b"prior-sidecar");

    let write_plan = plan_import(
        &uri(),
        VALID,
        &ImportContext::default(),
        ImportMode::Write,
        None,
        None,
    );
    let invalid_sidecar = root.join("sidecar-directory");
    fs::create_dir(&invalid_sidecar).unwrap();
    assert!(matches!(
        publish_directory_with_sidecar_atomically(
            &write_plan,
            &output,
            &invalid_sidecar,
            b"next-sidecar"
        ),
        Err(PublicationError::SidecarTargetIsNotFile(_))
    ));
    assert_eq!(fs::read(output.join("prior.txt")).unwrap(), b"prior-output");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sidecar_nested_in_output_is_not_reported_as_successfully_published() {
    let root = temp_directory("nested-sidecar");
    let output = root.join("imported");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("prior.txt"), b"prior-output").unwrap();
    let sidecar = output.join("source.meta");
    fs::write(&sidecar, b"prior-sidecar").unwrap();
    let plan = plan_import(
        &uri(),
        VALID,
        &ImportContext::default(),
        ImportMode::Write,
        None,
        None,
    );

    assert!(matches!(
        publish_directory_with_sidecar_atomically(&plan, &output, &sidecar, b"next-sidecar"),
        Err(PublicationError::OverlappingTargets { .. })
    ));
    assert_eq!(fs::read(output.join("prior.txt")).unwrap(), b"prior-output");
    assert_eq!(fs::read(&sidecar).unwrap(), b"prior-sidecar");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dry_run_cannot_be_published() {
    let root = temp_directory("dry-run");
    let output = root.join("imported");
    let plan = plan_import(
        &uri(),
        VALID,
        &ImportContext::default(),
        ImportMode::DryRun,
        None,
        None,
    );
    assert!(matches!(
        publish_directory_atomically(&plan, &output),
        Err(PublicationError::DryRun)
    ));
    assert!(!output.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_initializes_validates_plans_and_publishes_offline() {
    let root = temp_directory("cli");
    let source = root.join("fixture-triangle.mesh.json");
    let output = root.join("imported");
    fs::write(&source, VALID).unwrap();
    let binary = env!("CARGO_BIN_EXE_rusty-asset-import");

    let init = Command::new(binary)
        .arg("init-sidecar")
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let validate = Command::new(binary)
        .arg("validate-sidecar")
        .arg(&source)
        .output()
        .unwrap();
    assert!(validate.status.success());
    assert!(String::from_utf8_lossy(&validate.stdout).contains("status unchanged"));

    let dry_run = Command::new(binary)
        .arg("plan")
        .arg(&source)
        .arg(&output)
        .output()
        .unwrap();
    assert!(dry_run.status.success());
    assert!(!output.exists());
    let write = Command::new(binary)
        .arg("write")
        .arg(&source)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        write.status.success(),
        "{}",
        String::from_utf8_lossy(&write.stderr)
    );
    assert!(output.join("fixture-triangle.import.json").is_file());
    assert!(output.join("fixture-triangle.static-mesh.json").is_file());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_publishes_animated_glb_without_utf8_or_original_path_dependency() {
    let root = temp_directory("animated-cli");
    let source = root.join("actor-medium.glb");
    let output = root.join("imported");
    let asset_stem = animated_asset_stem(ANIMATED_GLB);
    fs::write(&source, ANIMATED_GLB).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rusty-asset-import"))
        .arg("write")
        .arg(&source)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read(output.join("actor-medium.glb")).unwrap(),
        ANIMATED_GLB
    );
    assert!(output
        .join(format!("{asset_stem}.animated-mesh.json"))
        .is_file());
    assert!(output.join(format!("{asset_stem}.catalog.json")).is_file());
    assert!(output.join(format!("{asset_stem}.import.json")).is_file());
    fs::remove_file(source).unwrap();
    let descriptor: AnimatedMeshAsset = serde_json::from_slice(
        &fs::read(output.join(format!("{asset_stem}.animated-mesh.json"))).unwrap(),
    )
    .unwrap();
    descriptor.validate().unwrap();
    assert_eq!(descriptor.asset, format!("mesh-animation/{asset_stem}"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_loads_gltf_closure_and_missing_resource_failure_preserves_publication() {
    let root = temp_directory("gltf-cli");
    let source = root.join("actor-external.gltf");
    let output = root.join("imported");
    let closure = external_gltf(
        ANIMATED_GLB,
        "buffers/actor.bin",
        Some("textures/actor.png"),
    );
    fs::write(&source, &closure.root_json).unwrap();
    for resource in &closure.resources {
        let path = root.join(&resource.uri);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, &resource.bytes).unwrap();
    }
    let binary = env!("CARGO_BIN_EXE_rusty-asset-import");
    let initialized = Command::new(binary)
        .arg("init-sidecar")
        .arg(&source)
        .output()
        .unwrap();
    assert!(initialized.status.success());
    let unchanged = Command::new(binary)
        .arg("validate-sidecar")
        .arg(&source)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&unchanged.stdout).contains("status unchanged"));
    let buffer_path = root.join("buffers/actor.bin");
    let original_buffer = fs::read(&buffer_path).unwrap();
    let mut changed_buffer = original_buffer.clone();
    *changed_buffer.last_mut().unwrap() ^= 1;
    fs::write(&buffer_path, &changed_buffer).unwrap();
    let changed = Command::new(binary)
        .arg("validate-sidecar")
        .arg(&source)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&changed.stdout).contains("status contentChanged"));
    fs::write(&buffer_path, original_buffer).unwrap();
    let result = Command::new(binary)
        .arg("write")
        .arg(&source)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.join("actor-external.glb").is_file());
    let runtime_before = fs::read(output.join("actor-external.glb")).unwrap();
    let asset_stem = animated_asset_stem(&runtime_before);
    let manifest_path = format!("{asset_stem}.import.json");
    let manifest_before = fs::read(output.join(&manifest_path)).unwrap();

    fs::remove_file(root.join("textures/actor.png")).unwrap();
    let rejected = Command::new(binary)
        .arg("write")
        .arg(&source)
        .arg(&output)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("referenced resource"));
    assert_eq!(
        fs::read(output.join(&manifest_path)).unwrap(),
        manifest_before
    );
    assert_eq!(
        fs::read(output.join("actor-external.glb")).unwrap(),
        runtime_before
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_rejects_oversized_sources_before_publishing() {
    let root = temp_directory("oversized-source");
    let source = root.join("oversized.mesh.json");
    let output = root.join("imported");
    let file = fs::File::create(&source).unwrap();
    file.set_len((MAX_SOURCE_BYTES + 1) as u64).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_rusty-asset-import"))
        .arg("write")
        .arg(&source)
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("admission limit"));
    assert!(!output.exists());

    fs::remove_dir_all(root).unwrap();
}

fn temp_directory(tag: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("rusty-asset-import-{tag}-{}", std::process::id()));
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
    fs::create_dir(&path).unwrap();
    path
}

fn artifact<'a>(plan: &'a ImportPlan, path: &str) -> &'a GeneratedArtifact {
    plan.files
        .iter()
        .find(|artifact| artifact.relative_path == path)
        .unwrap_or_else(|| panic!("missing artifact {path}"))
}

fn test_glb(json: &str, bin: &[u8]) -> Vec<u8> {
    let mut json = json.as_bytes().to_vec();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut bin = bin.to_vec();
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(b"glTF");
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&(total as u32).to_le_bytes());
    bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&0x4e4f_534au32.to_le_bytes());
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&0x004e_4942u32.to_le_bytes());
    bytes.extend_from_slice(&bin);
    bytes
}

fn rewrite_glb_json(source: &[u8], mutate: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    assert_eq!(&source[..4], b"glTF");
    let json_length = u32::from_le_bytes(source[12..16].try_into().unwrap()) as usize;
    let old_json_end = 20 + json_length;
    let mut root: serde_json::Value = serde_json::from_slice(&source[20..old_json_end]).unwrap();
    mutate(&mut root);
    let mut json = serde_json::to_vec(&root).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + source.len() - old_json_end;
    let mut rewritten = Vec::with_capacity(total);
    rewritten.extend_from_slice(b"glTF");
    rewritten.extend_from_slice(&2u32.to_le_bytes());
    rewritten.extend_from_slice(&(total as u32).to_le_bytes());
    rewritten.extend_from_slice(&(json.len() as u32).to_le_bytes());
    rewritten.extend_from_slice(&0x4e4f_534au32.to_le_bytes());
    rewritten.extend_from_slice(&json);
    rewritten.extend_from_slice(&source[old_json_end..]);
    rewritten
}

fn static_triangle_glb() -> Vec<u8> {
    let mut bin = Vec::new();
    for value in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        bin.extend_from_slice(&value.to_le_bytes());
    }
    for index in [0u16, 1, 2] {
        bin.extend_from_slice(&index.to_le_bytes());
    }
    bin.extend_from_slice(&[0, 0]);
    test_glb(
        r#"{
          "asset":{"version":"2.0"},
          "scene":0,
          "scenes":[{"nodes":[0]}],
          "nodes":[{"mesh":0}],
          "buffers":[{"byteLength":44}],
          "bufferViews":[
            {"buffer":0,"byteOffset":0,"byteLength":36,"target":34962},
            {"buffer":0,"byteOffset":36,"byteLength":6,"target":34963}
          ],
          "accessors":[
            {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},
            {"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}
          ],
          "meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1,"mode":4}]}]
        }"#,
        &bin,
    )
}

fn static_unlit_glb() -> Vec<u8> {
    BASE64.decode(STATIC_UNLIT_GLB_BASE64.trim()).unwrap()
}

fn external_gltf(glb: &[u8], buffer_uri: &str, image_uri: Option<&str>) -> GltfSourceClosure {
    assert_eq!(&glb[..4], b"glTF");
    let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let json_end = 20 + json_length;
    let mut root: serde_json::Value = serde_json::from_slice(&glb[20..json_end]).unwrap();
    let bin_header = json_end;
    assert_eq!(
        u32::from_le_bytes(glb[bin_header + 4..bin_header + 8].try_into().unwrap()),
        0x004e_4942
    );
    let bin_start = bin_header + 8;
    let declared_buffer_length = root["buffers"][0]["byteLength"].as_u64().unwrap() as usize;
    let bin = glb[bin_start..bin_start + declared_buffer_length].to_vec();
    root["buffers"][0]["uri"] = serde_json::Value::String(buffer_uri.to_owned());
    let mut resources = vec![GltfResource {
        uri: buffer_uri.to_owned(),
        bytes: bin.clone(),
    }];
    if let Some(image_uri) = image_uri {
        let view_index = root["images"][0]["bufferView"].as_u64().unwrap() as usize;
        let view = &root["bufferViews"][view_index];
        let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let length = view["byteLength"].as_u64().unwrap() as usize;
        let image_bytes = bin[offset..offset + length].to_vec();
        let image = root["images"][0].as_object_mut().unwrap();
        image.remove("bufferView");
        image.insert(
            "uri".to_owned(),
            serde_json::Value::String(image_uri.to_owned()),
        );
        resources.push(GltfResource {
            uri: image_uri.to_owned(),
            bytes: image_bytes,
        });
        resources.sort_by(|left, right| left.uri.cmp(&right.uri));
    }
    GltfSourceClosure {
        root_json: serde_json::to_vec(&root).unwrap(),
        resources,
    }
}
