use std::fs;
use std::path::{Path, PathBuf};

use asset_import::{
    decode_import_manifest, decode_sidecar, encode_sidecar, init_metadata, plan_import,
    publish_directory_atomically, reconcile, sidecar_path, ImportContext, ImportMode,
    ImportSettings, SourceUri, IMPORTER_VERSION,
};

fn main() {
    if let Err(error) = run(std::env::args().skip(1).collect()) {
        eprintln!("rusty-asset-import: {error}");
        std::process::exit(2);
    }
}

fn run(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    match args.as_slice() {
        [command, source, output] if command == "plan" || command == "write" => {
            import_command(command, Path::new(source), Path::new(output))
        }
        [command, source] if command == "init-sidecar" => {
            init_sidecar(Path::new(source), None, "cli")
        }
        [command, source, sidecar] if command == "init-sidecar" => {
            init_sidecar(Path::new(source), Some(Path::new(sidecar)), "cli")
        }
        [command, source] if command == "validate-sidecar" => {
            validate_sidecar(Path::new(source), None)
        }
        [command, source, sidecar] if command == "validate-sidecar" => {
            validate_sidecar(Path::new(source), Some(Path::new(sidecar)))
        }
        _ => Err(usage().into()),
    }
}

fn import_command(
    command: &str,
    source_path: &Path,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let source_text = fs::read_to_string(source_path)?;
    let source_uri = source_uri(source_path);
    let metadata_path = PathBuf::from(sidecar_path(&source_path.to_string_lossy()));
    let sidecar = if metadata_path.is_file() {
        Some(decode_sidecar(&fs::read_to_string(&metadata_path)?)?)
    } else {
        None
    };
    let context = ImportContext {
        available_textures: None,
        settings: sidecar
            .as_ref()
            .map_or_else(ImportSettings::default, |value| {
                value.import_settings.clone()
            }),
    };
    let mode = if command == "write" {
        ImportMode::Write
    } else {
        ImportMode::DryRun
    };
    let provisional = plan_import(
        &source_uri,
        &source_text,
        &context,
        mode,
        None,
        sidecar.as_ref(),
    );
    let prior = provisional
        .files
        .iter()
        .find(|file| file.relative_path.ends_with(".import.json"))
        .and_then(|file| fs::read_to_string(output.join(&file.relative_path)).ok())
        .and_then(|text| decode_import_manifest(&text).ok());
    let plan = plan_import(
        &source_uri,
        &source_text,
        &context,
        mode,
        prior.as_ref(),
        sidecar.as_ref(),
    );
    print!("{}", plan.report);
    if plan.has_errors {
        return Err("source admission failed".into());
    }
    if mode == ImportMode::Write {
        let receipt = publish_directory_atomically(&plan, output)?;
        println!(
            "published: {} ({} files, replacedPrevious={})",
            receipt.output_directory.display(),
            receipt.written_files.len(),
            receipt.replaced_previous
        );
        if let (Some(update), true) = (plan.sidecar_update.as_ref(), metadata_path.is_file()) {
            match encode_sidecar(update)
                .map_err(|error| error.to_string())
                .and_then(|encoded| {
                    write_file_atomically(&metadata_path, encoded.as_bytes())
                        .map_err(|error| error.to_string())
                }) {
                Ok(()) => println!("sidecar: updated {}", metadata_path.display()),
                Err(error) => eprintln!(
                    "sidecar warning: output publication succeeded but metadata update failed: {error}"
                ),
            }
        }
        if let Some(path) = receipt.retained_backup {
            eprintln!(
                "cleanup warning: previous output remains recoverable at {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn init_sidecar(
    source_path: &Path,
    explicit: Option<&Path>,
    salt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(source_path)?;
    let path = explicit
        .map(Path::to_owned)
        .unwrap_or_else(|| PathBuf::from(sidecar_path(&source_path.to_string_lossy())));
    if path.exists() {
        return Err(format!("sidecar already exists: {}", path.display()).into());
    }
    let metadata = init_metadata(
        source_uri(source_path),
        &bytes,
        "mesh",
        IMPORTER_VERSION,
        ImportSettings::default(),
        &format!(
            "{salt}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ),
    );
    write_file_atomically(&path, encode_sidecar(&metadata)?.as_bytes())?;
    println!(
        "initialized {} guid={}",
        path.display(),
        metadata.guid.as_str()
    );
    Ok(())
}

fn validate_sidecar(
    source_path: &Path,
    explicit: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = explicit
        .map(Path::to_owned)
        .unwrap_or_else(|| PathBuf::from(sidecar_path(&source_path.to_string_lossy())));
    if !path.is_file() {
        println!("status missingSidecar");
        return Ok(());
    }
    let metadata = decode_sidecar(&fs::read_to_string(path)?)?;
    let status = reconcile(
        Some(&metadata),
        &source_uri(source_path),
        &fs::read(source_path)?,
    );
    println!("status {} guid={}", status.label(), metadata.guid.as_str());
    Ok(())
}

fn source_uri(path: &Path) -> SourceUri {
    if path.is_absolute() {
        SourceUri::AbsolutePath(path.to_string_lossy().into_owned())
    } else {
        SourceUri::RelativePath(path.to_string_lossy().into_owned())
    }
}

fn write_file_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path"))?;
    let temporary = path.with_file_name(format!(".{file_name}.tmp-{}", std::process::id()));
    fs::write(&temporary, bytes)?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

fn usage() -> &'static str {
    "usage:\n  rusty-asset-import plan <source.mesh.json> <output-dir>\n  rusty-asset-import write <source.mesh.json> <output-dir>\n  rusty-asset-import init-sidecar <source> [sidecar]\n  rusty-asset-import validate-sidecar <source> [sidecar]"
}
