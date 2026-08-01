use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use asset_import::{
    admit_gltf_source, decode_import_manifest, decode_sidecar, encode_sidecar,
    gltf_relative_resource_uris, init_metadata, init_metadata_with_source_hash,
    plan_animated_glb_import, plan_animated_gltf_import, plan_import, publish_directory_atomically,
    publish_directory_with_sidecar_atomically, reconcile, reconcile_source_hash, sidecar_path,
    GltfResource, GltfSourceClosure, ImportContext, ImportMode, ImportPlan, ImportSettings,
    SourceUri, IMPORTER_VERSION, MAX_GLTF_RESOURCE_BYTES, MAX_GLTF_TOTAL_RESOURCE_BYTES,
    MAX_SOURCE_BYTES,
};

const MAX_AUXILIARY_BYTES: usize = 4 * 1024 * 1024;

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
    let source_bytes = read_bounded(source_path, MAX_SOURCE_BYTES)?;
    let gltf_source = is_extension(source_path, "gltf")
        .then(|| load_gltf_source_closure(source_path, source_bytes.clone()))
        .transpose()?;
    let source_uri = source_uri(source_path);
    let metadata_path = PathBuf::from(sidecar_path(&source_path.to_string_lossy()));
    let sidecar = if metadata_path.is_file() {
        Some(decode_sidecar(&read_bounded_text(
            &metadata_path,
            MAX_AUXILIARY_BYTES,
        )?)?)
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
    let provisional = plan_source(
        &source_uri,
        source_path,
        loaded_source(&source_bytes, gltf_source.as_ref()),
        &context,
        mode,
        None,
        sidecar.as_ref(),
    )?;
    let prior = provisional
        .files
        .iter()
        .find(|file| file.relative_path.ends_with(".import.json"))
        .and_then(|file| {
            read_bounded_text(&output.join(&file.relative_path), MAX_AUXILIARY_BYTES).ok()
        })
        .and_then(|text| decode_import_manifest(&text).ok());
    let plan = plan_source(
        &source_uri,
        source_path,
        loaded_source(&source_bytes, gltf_source.as_ref()),
        &context,
        mode,
        prior.as_ref(),
        sidecar.as_ref(),
    )?;
    print!("{}", plan.report);
    if plan.has_errors {
        return Err("source admission failed".into());
    }
    if mode == ImportMode::Write {
        let encoded_sidecar = plan
            .sidecar_update
            .as_ref()
            .filter(|_| metadata_path.is_file())
            .map(encode_sidecar)
            .transpose()?;
        let receipt = match encoded_sidecar.as_deref() {
            Some(encoded) => publish_directory_with_sidecar_atomically(
                &plan,
                output,
                &metadata_path,
                encoded.as_bytes(),
            )?,
            None => publish_directory_atomically(&plan, output)?,
        };
        println!(
            "published: {} ({} files, replacedPrevious={})",
            receipt.output_directory.display(),
            receipt.written_files.len(),
            receipt.replaced_previous
        );
        if encoded_sidecar.is_some() {
            println!("sidecar: updated {}", metadata_path.display());
        }
        if let Some(path) = receipt.retained_backup {
            eprintln!(
                "cleanup warning: previous output remains recoverable at {}",
                path.display()
            );
        }
        if let Some(path) = receipt.retained_sidecar_backup {
            eprintln!(
                "cleanup warning: previous sidecar remains recoverable at {}",
                path.display()
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum LoadedSource<'a> {
    Single(&'a [u8]),
    Gltf(&'a GltfSourceClosure),
}

fn loaded_source<'a>(bytes: &'a [u8], gltf: Option<&'a GltfSourceClosure>) -> LoadedSource<'a> {
    gltf.map_or(LoadedSource::Single(bytes), LoadedSource::Gltf)
}

fn plan_source(
    source_uri: &SourceUri,
    source_path: &Path,
    source: LoadedSource<'_>,
    context: &ImportContext,
    mode: ImportMode,
    prior: Option<&asset_import::ImportManifest>,
    sidecar: Option<&asset_import::SidecarMetadata>,
) -> Result<ImportPlan, Box<dyn std::error::Error>> {
    if is_extension(source_path, "glb") {
        let LoadedSource::Single(source_bytes) = source else {
            return Err("GLB source was loaded as a glTF closure".into());
        };
        return Ok(plan_animated_glb_import(
            source_uri,
            source_bytes,
            context,
            mode,
            prior,
            sidecar,
        ));
    }
    if is_extension(source_path, "gltf") {
        let LoadedSource::Gltf(gltf_source) = source else {
            return Err("glTF source closure was not loaded".into());
        };
        return Ok(plan_animated_gltf_import(
            source_uri,
            gltf_source,
            context,
            mode,
            prior,
            sidecar,
        ));
    }
    let LoadedSource::Single(source_bytes) = source else {
        return Err("text source was loaded as a glTF closure".into());
    };
    let source_text = std::str::from_utf8(source_bytes).map_err(|error| {
        format!(
            "{} is not UTF-8 and is not a .glb or .gltf source: {error}",
            source_path.display()
        )
    })?;
    Ok(plan_import(
        source_uri,
        source_text,
        context,
        mode,
        prior,
        sidecar,
    ))
}

fn init_sidecar(
    source_path: &Path,
    explicit: Option<&Path>,
    salt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = read_bounded(source_path, MAX_SOURCE_BYTES)?;
    let path = explicit
        .map(Path::to_owned)
        .unwrap_or_else(|| PathBuf::from(sidecar_path(&source_path.to_string_lossy())));
    if path.exists() {
        return Err(format!("sidecar already exists: {}", path.display()).into());
    }
    let declared_kind = if is_extension(source_path, "glb") || is_extension(source_path, "gltf") {
        "mesh-animation"
    } else {
        "mesh"
    };
    let uniqueness_salt = format!(
        "{salt}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let uri = source_uri(source_path);
    let metadata = if is_extension(source_path, "gltf") {
        let closure = load_gltf_source_closure(source_path, bytes)?;
        let packed = admit_gltf_source(&closure).map_err(|diagnostic| diagnostic.render())?;
        init_metadata_with_source_hash(
            uri,
            packed.source_hash,
            declared_kind,
            IMPORTER_VERSION,
            ImportSettings::default(),
            &uniqueness_salt,
        )
    } else {
        init_metadata(
            uri,
            &bytes,
            declared_kind,
            IMPORTER_VERSION,
            ImportSettings::default(),
            &uniqueness_salt,
        )
    };
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
    let metadata = decode_sidecar(&read_bounded_text(&path, MAX_AUXILIARY_BYTES)?)?;
    let uri = source_uri(source_path);
    let bytes = read_bounded(source_path, MAX_SOURCE_BYTES)?;
    let status = if is_extension(source_path, "gltf") {
        let closure = load_gltf_source_closure(source_path, bytes)?;
        let packed = admit_gltf_source(&closure).map_err(|diagnostic| diagnostic.render())?;
        reconcile_source_hash(Some(&metadata), &uri, packed.source_hash)
    } else {
        reconcile(Some(&metadata), &uri, &bytes)
    };
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

fn read_bounded(path: &Path, limit: usize) -> std::io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "{} exceeds the {limit}-byte admission limit",
                path.display()
            ),
        ));
    }
    Ok(bytes)
}

fn read_bounded_text(path: &Path, limit: usize) -> std::io::Result<String> {
    String::from_utf8(read_bounded(path, limit)?).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} is not UTF-8: {error}", path.display()),
        )
    })
}

fn load_gltf_source_closure(
    source_path: &Path,
    root_json: Vec<u8>,
) -> Result<GltfSourceClosure, Box<dyn std::error::Error>> {
    load_gltf_source_closure_with_limits(
        source_path,
        root_json,
        MAX_GLTF_RESOURCE_BYTES,
        MAX_GLTF_TOTAL_RESOURCE_BYTES,
    )
}

fn load_gltf_source_closure_with_limits(
    source_path: &Path,
    root_json: Vec<u8>,
    per_resource_limit: usize,
    aggregate_limit: usize,
) -> Result<GltfSourceClosure, Box<dyn std::error::Error>> {
    let resource_uris =
        gltf_relative_resource_uris(&root_json).map_err(|diagnostic| diagnostic.render())?;
    let parent = source_path.parent().unwrap_or_else(|| Path::new("."));
    let canonical_parent = fs::canonicalize(parent)?;
    let mut resources = Vec::with_capacity(resource_uris.len());
    let mut retained_bytes = 0usize;
    for uri in resource_uris {
        let candidate = canonical_parent.join(&uri);
        let canonical = fs::canonicalize(&candidate).map_err(|error| {
            format!("source.resources[{uri}]: referenced resource could not be opened: {error}")
        })?;
        if !canonical.starts_with(&canonical_parent) || !canonical.is_file() {
            return Err(format!(
                "source.resources[{uri}]: resource resolves outside the source directory or is not a file"
            )
            .into());
        }
        let remaining = aggregate_limit.checked_sub(retained_bytes).ok_or_else(|| {
            format!(
                "source.resources: total resource bytes exceed the {aggregate_limit}-byte admission limit"
            )
        })?;
        let read_limit = per_resource_limit.min(remaining);
        let bytes = read_bounded(&canonical, read_limit).map_err(|error| {
            if error.kind() == std::io::ErrorKind::InvalidData {
                format!(
                    "source.resources[{uri}]: resource exceeds its remaining bounded allowance: {error}"
                )
            } else {
                format!("source.resources[{uri}]: resource could not be read: {error}")
            }
        })?;
        retained_bytes = retained_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| "source.resources: total resource byte count overflowed".to_owned())?;
        resources.push(GltfResource { uri, bytes });
    }
    Ok(GltfSourceClosure {
        root_json,
        resources,
    })
}

fn is_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn usage() -> &'static str {
    "usage:\n  rusty-asset-import plan <source.mesh.json|source.glb|source.gltf> <output-dir>\n  rusty-asset-import write <source.mesh.json|source.glb|source.gltf> <output-dir>\n  rusty-asset-import init-sidecar <source> [sidecar]\n  rusty-asset-import validate-sidecar <source> [sidecar]"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gltf_loader_accepts_exact_aggregate_and_rejects_one_over_before_retention() {
        let root = std::env::temp_dir().join(format!(
            "rusty-asset-import-gltf-loader-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let source = root.join("bounded.gltf");
        let document = br#"{
          "asset":{"version":"2.0"},
          "buffers":[
            {"uri":"first.bin","byteLength":4},
            {"uri":"second.bin","byteLength":4}
          ]
        }"#;
        fs::write(&source, document).unwrap();
        fs::write(root.join("first.bin"), [1, 2, 3, 4]).unwrap();
        fs::write(root.join("second.bin"), [5, 6, 7, 8]).unwrap();
        let exact = load_gltf_source_closure_with_limits(&source, document.to_vec(), 4, 8)
            .expect("the exact aggregate limit is admitted");
        assert_eq!(
            exact
                .resources
                .iter()
                .map(|resource| resource.bytes.len())
                .sum::<usize>(),
            8
        );

        fs::write(root.join("second.bin"), [5, 6, 7, 8, 9]).unwrap();
        let sentinel = root.join("published.sentinel");
        fs::write(&sentinel, b"unchanged").unwrap();
        let failure = load_gltf_source_closure_with_limits(&source, document.to_vec(), 5, 8)
            .expect_err("one byte over the aggregate limit must reject");
        assert!(failure.to_string().contains("remaining bounded allowance"));
        assert_eq!(fs::read(&sentinel).unwrap(), b"unchanged");
        fs::remove_dir_all(root).unwrap();
    }
}
