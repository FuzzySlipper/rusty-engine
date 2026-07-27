use std::io::{Read, Write};
use std::process::ExitCode;

use asset_import::ImportContext;
use engine_inspector::{
    entity_ids_in_category, inspect_catalog_json, inspect_content_manifest_json, inspect_entity,
    inspect_entity_state_json, inspect_import_manifest_json, inspect_import_source,
    inspect_mechanics_snapshot_json, inspect_scene_json, inspect_voxel_asset_json, DiagnosticSet,
    EntityCategory,
};

const MAX_CATALOG_BYTES: usize = 16 * 1024 * 1024;
const MAX_SCENE_BYTES: usize = 64 * 1024 * 1024;
const MAX_ENTITY_STATE_BYTES: usize = 64 * 1024 * 1024;
const MAX_IMPORT_MANIFEST_BYTES: usize = 4 * 1024 * 1024;

const USAGE: &str = "\
rusty-inspect — read-only Rusty Engine content and state inspection

USAGE:
    rusty-inspect catalog <catalog.json> [asset-lock.json]
    rusty-inspect scene <scene.json> [catalog.json]
    rusty-inspect entity-state summary <entity-state.json>
    rusty-inspect entity-state entity <entity-state.json> <entity-id>
    rusty-inspect entity-state category <entity-state.json> <category>
    rusty-inspect mechanics <entity-state.json> <mechanics-catalog.json> <entity-id>
    rusty-inspect voxel <voxel-asset.json>
    rusty-inspect content <content-manifest.json>
    rusty-inspect import-source <source-mesh.json>
    rusty-inspect import-manifest <import-manifest.json>
    rusty-inspect --help

ENTITY CATEGORIES:
    all, active, disabled, tombstoned, spatial, non-spatial, rendered,
    colliding, contained, asset-bound

EXIT CODES:
    0 inspection completed without errors
    1 focused entity/category query had no result
    2 artifact read, decode, validation, or import failed
    3 command usage error
";

fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    ExitCode::from(run(&args, &mut std::io::stdout(), &mut std::io::stderr()))
}

fn run<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    match args.first().map(String::as_str) {
        Some("--help" | "-h" | "help") => {
            let _ = write!(out, "{USAGE}");
            0
        }
        None => {
            let _ = write!(err, "{USAGE}");
            3
        }
        Some("catalog") => command_catalog(&args[1..], out, err),
        Some("scene") => command_scene(&args[1..], out, err),
        Some("entity-state") => command_entity_state(&args[1..], out, err),
        Some("mechanics") => command_mechanics(&args[1..], out, err),
        Some("voxel") => command_voxel(&args[1..], out, err),
        Some("content") => command_content(&args[1..], out, err),
        Some("import-source") => command_import_source(&args[1..], out, err),
        Some("import-manifest") => command_import_manifest(&args[1..], out, err),
        Some(command) => {
            let _ = writeln!(err, "error: unknown command {command:?}\n");
            let _ = write!(err, "{USAGE}");
            3
        }
    }
}

fn command_mechanics<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    let [snapshot_path, catalog_path, entity] = args else {
        return usage_error(
            err,
            "`mechanics` requires <entity-state.json> <mechanics-catalog.json> <entity-id>",
        );
    };
    let Ok(entity) = entity.parse::<u64>() else {
        return usage_error(err, "entity id must be an unsigned integer");
    };
    let Some(snapshot) = read_text(snapshot_path, MAX_ENTITY_STATE_BYTES, err) else {
        return 2;
    };
    let Some(catalog) = read_text(catalog_path, MAX_CATALOG_BYTES, err) else {
        return 2;
    };
    match inspect_mechanics_snapshot_json(&snapshot, &catalog, entity) {
        Ok(report) => {
            let _ = write!(out, "{}", report.to_text());
            0
        }
        Err(diagnostics) => finish_failure(&diagnostics, err),
    }
}

fn command_catalog<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    let (catalog_path, lock_path) = match args {
        [catalog] => (catalog.as_str(), None),
        [catalog, lock] => (catalog.as_str(), Some(lock.as_str())),
        _ => return usage_error(err, "`catalog` requires <catalog.json> [asset-lock.json]"),
    };
    let Some(catalog) = read_text(catalog_path, MAX_CATALOG_BYTES, err) else {
        return 2;
    };
    let lock = match lock_path {
        Some(path) => match read_text(path, MAX_CATALOG_BYTES, err) {
            Some(text) => Some(text),
            None => return 2,
        },
        None => None,
    };
    match inspect_catalog_json(&catalog, lock.as_deref()) {
        Ok(report) => finish_report(report.to_text(), &report.diagnostics, out),
        Err(diagnostics) => finish_failure(&diagnostics, err),
    }
}

fn command_scene<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    let (scene_path, catalog_path) = match args {
        [scene] => (scene.as_str(), None),
        [scene, catalog] => (scene.as_str(), Some(catalog.as_str())),
        _ => return usage_error(err, "`scene` requires <scene.json> [catalog.json]"),
    };
    let Some(scene) = read_text(scene_path, MAX_SCENE_BYTES, err) else {
        return 2;
    };
    let catalog = match catalog_path {
        Some(path) => match read_text(path, MAX_CATALOG_BYTES, err) {
            Some(text) => Some(text),
            None => return 2,
        },
        None => None,
    };
    match inspect_scene_json(&scene, catalog.as_deref()) {
        Ok(report) => finish_report(report.to_text(), &report.diagnostics, out),
        Err(diagnostics) => finish_failure(&diagnostics, err),
    }
}

fn command_entity_state<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    let Some(operation) = args.first().map(String::as_str) else {
        return usage_error(err, "`entity-state` requires summary, entity, or category");
    };
    match operation {
        "summary" => {
            let [path] = &args[1..] else {
                return usage_error(err, "`entity-state summary` requires <entity-state.json>");
            };
            let Some(input) = read_text(path, MAX_ENTITY_STATE_BYTES, err) else {
                return 2;
            };
            match inspect_entity_state_json(&input) {
                Ok(report) => finish_report(report.to_text(), &report.diagnostics, out),
                Err(diagnostics) => finish_failure(&diagnostics, err),
            }
        }
        "entity" => {
            let [path, id] = &args[1..] else {
                return usage_error(
                    err,
                    "`entity-state entity` requires <entity-state.json> <entity-id>",
                );
            };
            let Ok(id) = id.parse::<u64>() else {
                return usage_error(err, "entity id must be an unsigned integer");
            };
            let Some(input) = read_text(path, MAX_ENTITY_STATE_BYTES, err) else {
                return 2;
            };
            if let Err(diagnostics) = inspect_entity_state_json(&input) {
                return finish_failure(&diagnostics, err);
            }
            let state = entity_state::decode_snapshot(&input)
                .expect("the inspection decode already accepted this snapshot");
            match inspect_entity(&state, id) {
                Some(report) => {
                    let _ = write!(out, "{}", report.to_text());
                    0
                }
                None => {
                    let _ = writeln!(err, "missing entity: {id}");
                    1
                }
            }
        }
        "category" => {
            let [path, category] = &args[1..] else {
                return usage_error(
                    err,
                    "`entity-state category` requires <entity-state.json> <category>",
                );
            };
            let Some(category) = EntityCategory::from_label(category) else {
                return usage_error(err, "unsupported entity category");
            };
            let Some(input) = read_text(path, MAX_ENTITY_STATE_BYTES, err) else {
                return 2;
            };
            if let Err(diagnostics) = inspect_entity_state_json(&input) {
                return finish_failure(&diagnostics, err);
            }
            let state = entity_state::decode_snapshot(&input)
                .expect("the inspection decode already accepted this snapshot");
            let ids = entity_ids_in_category(&state, category);
            if ids.is_empty() {
                let _ = writeln!(err, "empty category: {}", category.label());
                return 1;
            }
            let ids = ids.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
            let _ = writeln!(out, "category {}\nentity-ids [{ids}]", category.label());
            0
        }
        _ => usage_error(err, "`entity-state` requires summary, entity, or category"),
    }
}

fn command_voxel<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    let Some((path, input)) = one_input(args, "voxel", voxel_asset::MAX_ARTIFACT_BYTES, err) else {
        return if args.len() == 1 { 2 } else { 3 };
    };
    match inspect_voxel_asset_json(&input) {
        Ok(report) => finish_report(report.to_text(), &report.diagnostics, out),
        Err(diagnostics) => {
            let _ = writeln!(err, "artifact: {path}");
            finish_failure(&diagnostics, err)
        }
    }
}

fn command_content<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    let Some((_, input)) = one_input(
        args,
        "content",
        content_store::CONTENT_MANIFEST_MAX_BYTES,
        err,
    ) else {
        return if args.len() == 1 { 2 } else { 3 };
    };
    match inspect_content_manifest_json(&input) {
        Ok(report) => finish_report(report.to_text(), &report.diagnostics, out),
        Err(diagnostics) => finish_failure(&diagnostics, err),
    }
}

fn command_import_source<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    let Some((path, input)) = one_input(args, "import-source", asset_import::MAX_SOURCE_BYTES, err)
    else {
        return if args.len() == 1 { 2 } else { 3 };
    };
    let report = inspect_import_source(&input, path, &ImportContext::default());
    finish_report(report.to_text(), &report.diagnostics, out)
}

fn command_import_manifest<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    let Some((_, input)) = one_input(args, "import-manifest", MAX_IMPORT_MANIFEST_BYTES, err)
    else {
        return if args.len() == 1 { 2 } else { 3 };
    };
    match inspect_import_manifest_json(&input) {
        Ok(report) => finish_report(report.to_text(), &report.diagnostics, out),
        Err(diagnostics) => finish_failure(&diagnostics, err),
    }
}

fn one_input<'a, E: Write>(
    args: &'a [String],
    command: &str,
    max_bytes: usize,
    err: &mut E,
) -> Option<(&'a str, String)> {
    let [path] = args else {
        let _ = writeln!(err, "error: `{command}` requires one artifact path");
        return None;
    };
    read_text(path, max_bytes, err).map(|input| (path.as_str(), input))
}

fn read_text<E: Write>(path: &str, max_bytes: usize, err: &mut E) -> Option<String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) => {
            let _ = writeln!(err, "error: cannot read {path}: {error}");
            return None;
        }
    };
    if file
        .metadata()
        .is_ok_and(|metadata| metadata.len() > max_bytes as u64)
    {
        let _ = writeln!(
            err,
            "error: {path} exceeds the {max_bytes}-byte inspection read limit"
        );
        return None;
    }
    let mut bytes = Vec::new();
    if let Err(error) = file.take(max_bytes as u64 + 1).read_to_end(&mut bytes) {
        let _ = writeln!(err, "error: cannot read {path}: {error}");
        return None;
    }
    if bytes.len() > max_bytes {
        let _ = writeln!(
            err,
            "error: {path} exceeds the {max_bytes}-byte inspection read limit"
        );
        return None;
    }
    match String::from_utf8(bytes) {
        Ok(input) => Some(input),
        Err(error) => {
            let _ = writeln!(err, "error: {path} is not valid UTF-8: {error}");
            None
        }
    }
}

fn finish_report<O: Write>(text: String, diagnostics: &DiagnosticSet, out: &mut O) -> u8 {
    let _ = write!(out, "{text}");
    u8::from(diagnostics.has_errors()) * 2
}

fn finish_failure<E: Write>(diagnostics: &DiagnosticSet, err: &mut E) -> u8 {
    let _ = write!(err, "{}", diagnostics.to_text());
    2
}

fn usage_error<E: Write>(err: &mut E, message: &str) -> u8 {
    let _ = writeln!(err, "error: {message}");
    3
}

#[cfg(test)]
mod tests {
    use core_ids::EntityId;
    use core_math::Vec3;
    use entity_state::{encode_snapshot, EntityDefinition, EntityState};

    use super::*;

    fn run_text(args: &[&str]) -> (u8, String, String) {
        let args = args
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run(&args, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn temporary_file(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rusty-inspect-{name}-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn help_is_stable_and_lists_every_owner() {
        let (code, out, err) = run_text(&["--help"]);
        assert_eq!(code, 0);
        assert!(err.is_empty());
        for command in [
            "catalog",
            "scene",
            "entity-state",
            "mechanics",
            "voxel",
            "content",
            "import-source",
            "import-manifest",
        ] {
            assert!(out.contains(command));
        }
    }

    #[test]
    fn successor_entity_snapshot_supports_summary_focus_and_category() {
        let state = EntityState::from_definitions([
            EntityDefinition::new(EntityId::new(1), "root").with_transform(Vec3::ZERO),
            EntityDefinition::new(EntityId::new(2), "child")
                .with_transform(Vec3::ONE)
                .with_transform_parent(EntityId::new(1)),
        ])
        .unwrap();
        let path = temporary_file("entity", &encode_snapshot(&state).unwrap());
        let path = path.to_string_lossy();

        let (code, out, err) = run_text(&["entity-state", "summary", &path]);
        assert_eq!(code, 0);
        assert!(err.is_empty());
        assert!(out.contains("entities=2"));

        let (code, out, err) = run_text(&["entity-state", "entity", &path, "2"]);
        assert_eq!(code, 0);
        assert!(err.is_empty());
        assert!(out.contains("relationships [transformParent=1]"));

        let (code, out, err) = run_text(&["entity-state", "category", &path, "spatial"]);
        assert_eq!(code, 0);
        assert!(err.is_empty());
        assert!(out.contains("entity-ids [1,2]"));

        std::fs::remove_file(path.as_ref()).ok();
    }

    #[test]
    fn malformed_artifact_exits_two_with_structured_diagnostic() {
        let path = temporary_file("bad", "{ nope");
        let path = path.to_string_lossy();
        let (code, out, err) = run_text(&["content", &path]);
        assert_eq!(code, 2);
        assert!(out.is_empty());
        assert!(err.contains("[fatal] persistence contentManifest.decode"));
        std::fs::remove_file(path.as_ref()).ok();
    }

    #[test]
    fn mechanics_command_uses_strict_catalog_and_owner_diagnostics() {
        let snapshot = temporary_file("mechanics-state", "{}");
        let catalog = temporary_file("mechanics-catalog", "{}");
        let snapshot_text = snapshot.to_string_lossy();
        let catalog_text = catalog.to_string_lossy();
        let (code, out, err) = run_text(&["mechanics", &snapshot_text, &catalog_text, "1"]);
        assert_eq!(code, 2);
        assert!(out.is_empty());
        assert!(err.contains("[fatal] gameplayMechanics catalog.decode"));
        std::fs::remove_file(snapshot).ok();
        std::fs::remove_file(catalog).ok();
    }

    #[test]
    fn cli_rejects_oversized_artifacts_before_owner_decode() {
        let path = std::env::temp_dir().join(format!(
            "rusty-inspect-oversized-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(content_store::CONTENT_MANIFEST_MAX_BYTES as u64 + 1)
            .unwrap();
        let path = path.to_string_lossy();

        let (code, out, err) = run_text(&["content", &path]);

        assert_eq!(code, 2);
        assert!(out.is_empty());
        assert!(err.contains("inspection read limit"));
        std::fs::remove_file(path.as_ref()).ok();
    }
}
