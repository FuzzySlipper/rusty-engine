use std::fs;
use std::path::PathBuf;
use std::process::Command;

use asset_catalog::validate_catalog;
use asset_import::*;

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

fn uri() -> SourceUri {
    SourceUri::RelativePath("assets/fixture-triangle.mesh.json".to_owned())
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

fn temp_directory(tag: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("rusty-asset-import-{tag}-{}", std::process::id()));
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
    fs::create_dir(&path).unwrap();
    path
}
