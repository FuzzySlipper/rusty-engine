use std::{
    fs, io,
    io::Read,
    path::{Path, PathBuf},
};

use product_model::{
    decode_product_manifest, ProductManifest, ProductPath, MAX_PRODUCT_MANIFEST_BYTES,
};

use crate::{
    report::{Diagnostic, Report},
    Execution, EXIT_CONFORMANCE, EXIT_ROOT,
};

const MANIFEST_NAME: &str = "rusty.toml";
const GENERATED_LANE: &str = "generated";
const MAX_DISCOVERY_ANCESTORS: usize = 64;
const MAX_HOST_SCAN_ENTRIES: usize = 512;
const PROHIBITED_ROOT_PATHS: &[&str] = &[
    "index.html",
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "tsconfig.json",
    "src-tauri",
    "wrappers",
    "electron",
    "tauri",
];

pub(crate) fn check(start: PathBuf) -> Execution {
    let root = match discover_root(&start) {
        Ok(root) => root,
        Err(diagnostic) => return failure_execution(EXIT_ROOT, diagnostic),
    };
    let diagnostics = validate_layout(&root);
    let report = Report::checked(diagnostics);
    Execution {
        exit_code: if report.has_errors() {
            EXIT_CONFORMANCE
        } else {
            0
        },
        report,
    }
}

fn discover_root(start: &Path) -> Result<PathBuf, Diagnostic> {
    let metadata = fs::metadata(start).map_err(|error| {
        Diagnostic::error(
            "RUSTY_DISCOVERY_START",
            start.display().to_string(),
            format!("cannot inspect discovery start: {error}"),
        )
    })?;
    let start = if metadata.is_file() {
        start.parent().ok_or_else(|| {
            Diagnostic::error(
                "RUSTY_DISCOVERY_START",
                start.display().to_string(),
                "a file discovery start must have a parent directory",
            )
        })?
    } else if metadata.is_dir() {
        start
    } else {
        return Err(Diagnostic::error(
            "RUSTY_DISCOVERY_START",
            start.display().to_string(),
            "discovery start must be a directory or file within a product",
        ));
    };
    let mut current = fs::canonicalize(start).map_err(|error| {
        Diagnostic::error(
            "RUSTY_DISCOVERY_START",
            start.display().to_string(),
            format!("cannot resolve discovery start: {error}"),
        )
    })?;
    for _ in 0..MAX_DISCOVERY_ANCESTORS {
        let manifest = current.join(MANIFEST_NAME);
        match fs::symlink_metadata(&manifest) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Diagnostic::error(
                    "RUSTY_MANIFEST_SYMLINK",
                    MANIFEST_NAME,
                    "product discovery does not admit a symlinked rusty.toml",
                ));
            }
            Ok(metadata) if metadata.is_file() => return Ok(current),
            Ok(_) | Err(_) => {}
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
    Err(Diagnostic::error(
        "RUSTY_ROOT_NOT_FOUND",
        start.display().to_string(),
        format!("no {MANIFEST_NAME} found within {MAX_DISCOVERY_ANCESTORS} ancestors"),
    ))
}

fn validate_layout(root: &Path) -> Vec<Diagnostic> {
    let manifest_path = root.join(MANIFEST_NAME);
    let manifest_text = match read_bounded(&manifest_path, MAX_PRODUCT_MANIFEST_BYTES) {
        Ok(content) => content,
        Err(diagnostic) => return vec![diagnostic],
    };
    let manifest = match decode_product_manifest(&manifest_text) {
        Ok(manifest) => manifest,
        Err(error) => {
            return vec![Diagnostic::error(
                "RUSTY_MANIFEST_INVALID",
                MANIFEST_NAME,
                format!(
                    "{} at {}: {}",
                    error.diagnostic().code(),
                    error.diagnostic().path(),
                    error.diagnostic().message()
                ),
            )]
        }
    };
    let canonical_root = match fs::canonicalize(root) {
        Ok(root) => root,
        Err(error) => {
            return vec![Diagnostic::error(
                "RUSTY_ROOT_CANONICALIZE",
                "$",
                format!("cannot resolve product root: {error}"),
            )]
        }
    };
    let mut diagnostics = Vec::new();
    check_required_layout(&canonical_root, &manifest, &mut diagnostics);
    check_generated_ownership(&canonical_root, &manifest, &mut diagnostics);
    check_prohibited_host_paths(&canonical_root, &mut diagnostics);
    diagnostics
}

fn read_bounded(path: &Path, maximum: usize) -> Result<String, Diagnostic> {
    let link_metadata = fs::symlink_metadata(path).map_err(|error| {
        Diagnostic::error(
            "RUSTY_MANIFEST_READ",
            MANIFEST_NAME,
            format!("cannot read {MANIFEST_NAME}: {error}"),
        )
    })?;
    if link_metadata.file_type().is_symlink() {
        return Err(Diagnostic::error(
            "RUSTY_MANIFEST_SYMLINK",
            MANIFEST_NAME,
            format!("{MANIFEST_NAME} must be an in-root regular file, not a symlink"),
        ));
    }
    let metadata = fs::metadata(path).map_err(|error| {
        Diagnostic::error(
            "RUSTY_MANIFEST_READ",
            MANIFEST_NAME,
            format!("cannot read {MANIFEST_NAME}: {error}"),
        )
    })?;
    if !metadata.is_file() {
        return Err(Diagnostic::error(
            "RUSTY_MANIFEST_READ",
            MANIFEST_NAME,
            format!("{MANIFEST_NAME} must be a regular file"),
        ));
    }
    if metadata.len() > maximum as u64 {
        return Err(Diagnostic::error(
            "RUSTY_MANIFEST_BYTES_EXCEEDED",
            MANIFEST_NAME,
            format!("{MANIFEST_NAME} is limited to {maximum} bytes"),
        ));
    }
    let mut file = fs::File::open(path).map_err(|error| {
        Diagnostic::error(
            "RUSTY_MANIFEST_READ",
            MANIFEST_NAME,
            format!("cannot open {MANIFEST_NAME}: {error}"),
        )
    })?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            Diagnostic::error(
                "RUSTY_MANIFEST_READ",
                MANIFEST_NAME,
                format!("cannot read {MANIFEST_NAME}: {error}"),
            )
        })?;
    if bytes.len() > maximum {
        return Err(Diagnostic::error(
            "RUSTY_MANIFEST_BYTES_EXCEEDED",
            MANIFEST_NAME,
            format!("{MANIFEST_NAME} is limited to {maximum} bytes"),
        ));
    }
    String::from_utf8(bytes).map_err(|error| {
        Diagnostic::error(
            "RUSTY_MANIFEST_UTF8",
            MANIFEST_NAME,
            format!("{MANIFEST_NAME} must be UTF-8: {error}"),
        )
    })
}

fn check_required_layout(
    root: &Path,
    manifest: &ProductManifest,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entrypoint in manifest.composition_entrypoints() {
        require_extension(
            entrypoint,
            ".ts",
            "RUSTY_LAYOUT_RULES_EXTENSION",
            diagnostics,
        );
        require_contained_file(root, entrypoint, "RUSTY_LAYOUT_ENTRYPOINT", diagnostics);
    }
    if let Some(entrypoint) = manifest.kernel_entry() {
        require_extension(
            entrypoint,
            ".rs",
            "RUSTY_LAYOUT_KERNEL_EXTENSION",
            diagnostics,
        );
        require_contained_file(root, entrypoint, "RUSTY_LAYOUT_ENTRYPOINT", diagnostics);
    }
    if let Some(package) = manifest.kernel_package() {
        require_extension(
            package,
            "/Cargo.toml",
            "RUSTY_LAYOUT_KERNEL_PACKAGE_MANIFEST",
            diagnostics,
        );
        require_contained_file(root, package, "RUSTY_LAYOUT_KERNEL_PACKAGE", diagnostics);
        require_contained_directory(
            root,
            &ProductPath::parse("kernel".to_owned()).expect("fixed kernel lane"),
            "RUSTY_LAYOUT_KERNEL_PACKAGE",
            diagnostics,
        );
    }
    require_extension(
        manifest.ui_entry(),
        ".ts",
        "RUSTY_LAYOUT_UI_EXTENSION",
        diagnostics,
    );
    require_contained_file(
        root,
        manifest.ui_entry(),
        "RUSTY_LAYOUT_ENTRYPOINT",
        diagnostics,
    );
    require_contained_directory(
        root,
        manifest.content_root(),
        "RUSTY_LAYOUT_CONTENT",
        diagnostics,
    );
    require_contained_directory(
        root,
        &ProductPath::parse(GENERATED_LANE).expect("fixed lane is valid"),
        "RUSTY_LAYOUT_GENERATED",
        diagnostics,
    );
}

fn require_extension(
    relative: &ProductPath,
    expected: &str,
    code: &'static str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !relative.as_str().ends_with(expected) {
        diagnostics.push(Diagnostic::error(
            code,
            MANIFEST_NAME,
            format!("declared entrypoint must end in `{expected}`"),
        ));
    }
}

fn require_contained_file(
    root: &Path,
    relative: &ProductPath,
    family: &'static str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = root.join(relative.as_str());
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
            require_contained_target(root, &path, family, false, diagnostics)
        }
        Ok(_) => diagnostics.push(Diagnostic::error(
            "RUSTY_LAYOUT_ENTRYPOINT_NOT_FILE",
            relative_display(root, &path),
            format!("{family} must name a regular file"),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            diagnostics.push(Diagnostic::error(
                "RUSTY_LAYOUT_ENTRYPOINT_MISSING",
                relative.as_str(),
                "declared entrypoint is missing",
            ))
        }
        Err(error) => diagnostics.push(Diagnostic::error(
            "RUSTY_LAYOUT_ENTRYPOINT_READ",
            relative_display(root, &path),
            format!("cannot inspect declared entrypoint: {error}"),
        )),
    }
}

fn require_contained_directory(
    root: &Path,
    relative: &ProductPath,
    family: &'static str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = root.join(relative.as_str());
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_dir() => {
            require_contained_target(root, &path, family, true, diagnostics)
        }
        Ok(_) => diagnostics.push(Diagnostic::error(
            "RUSTY_LAYOUT_DIRECTORY_NOT_DIRECTORY",
            relative_display(root, &path),
            format!("{family} must name a directory"),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            diagnostics.push(Diagnostic::error(
                "RUSTY_LAYOUT_DIRECTORY_MISSING",
                relative.as_str(),
                "required fixed product lane is missing",
            ))
        }
        Err(error) => diagnostics.push(Diagnostic::error(
            "RUSTY_LAYOUT_DIRECTORY_READ",
            relative_display(root, &path),
            format!("cannot inspect required product lane: {error}"),
        )),
    }
}

fn require_contained_target(
    root: &Path,
    path: &Path,
    family: &'static str,
    directory: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match fs::canonicalize(path) {
        Ok(target) if !target.starts_with(root) => diagnostics.push(Diagnostic::error(
            "RUSTY_PATH_SYMLINK_ESCAPE",
            relative_display(root, path),
            format!("{family} resolves outside the product root"),
        )),
        Ok(target) if directory && !target.is_dir() => diagnostics.push(Diagnostic::error(
            "RUSTY_LAYOUT_DIRECTORY_NOT_DIRECTORY",
            relative_display(root, path),
            format!("{family} symlink does not resolve to a directory"),
        )),
        Ok(target) if !directory && !target.is_file() => diagnostics.push(Diagnostic::error(
            "RUSTY_LAYOUT_ENTRYPOINT_NOT_FILE",
            relative_display(root, path),
            format!("{family} symlink does not resolve to a regular file"),
        )),
        Ok(_) => {}
        Err(error) => diagnostics.push(Diagnostic::error(
            "RUSTY_PATH_SYMLINK_READ",
            relative_display(root, path),
            format!("cannot resolve symlink: {error}"),
        )),
    }
}

fn check_generated_ownership(
    root: &Path,
    manifest: &ProductManifest,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let generated = root.join(GENERATED_LANE);
    match fs::symlink_metadata(&generated) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            diagnostics.push(Diagnostic::error(
                "RUSTY_OUTPUT_GENERATED_LANE",
                GENERATED_LANE,
                "generated must be a real product-root directory, not a symlink or another artifact kind",
            ));
            return;
        }
        Ok(_) => match fs::canonicalize(&generated) {
            Ok(target) if target != generated || !target.starts_with(root) => {
                diagnostics.push(Diagnostic::error(
                    "RUSTY_OUTPUT_SYMLINK_ESCAPE",
                    GENERATED_LANE,
                    "generated output lane must resolve to its own canonical product-root location",
                ));
                return;
            }
            Ok(_) => {}
            Err(error) => diagnostics.push(Diagnostic::error(
                "RUSTY_OUTPUT_SYMLINK_READ",
                GENERATED_LANE,
                format!("cannot resolve generated output lane: {error}"),
            )),
        },
        Err(error) => diagnostics.push(Diagnostic::error(
            "RUSTY_OUTPUT_GENERATED_LANE",
            GENERATED_LANE,
            format!("cannot inspect generated lane: {error}"),
        )),
    }
    for output in [
        manifest.compiled_composition_output(),
        manifest.admitted_runtime_content_output(),
        manifest.product_assembly_output(),
        manifest.product_bundle_output(),
    ] {
        let path = root.join(output.as_str());
        if path.exists()
            || fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            require_generated_target(
                root,
                &path,
                output == manifest.compiled_composition_output(),
                diagnostics,
            );
        }
    }
}

fn require_generated_target(
    root: &Path,
    path: &Path,
    expects_file: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match fs::canonicalize(path) {
        Ok(target) if !target.starts_with(root.join(GENERATED_LANE)) => {
            diagnostics.push(Diagnostic::error(
                "RUSTY_OUTPUT_OUTSIDE_GENERATED",
                relative_display(root, path),
                "declared generated output resolves outside the generated lane",
            ))
        }
        Ok(target) if expects_file && !target.is_file() => diagnostics.push(Diagnostic::error(
            "RUSTY_OUTPUT_KIND",
            relative_display(root, path),
            "compiled composition output must be a regular file when it exists",
        )),
        Ok(target) if !expects_file && !target.is_dir() => diagnostics.push(Diagnostic::error(
            "RUSTY_OUTPUT_KIND",
            relative_display(root, path),
            "generated runtime content, assembly, and bundle outputs must be directories when they exist",
        )),
        Ok(_) => {}
        Err(error) => diagnostics.push(Diagnostic::error(
            "RUSTY_OUTPUT_SYMLINK_READ",
            relative_display(root, path),
            format!("cannot resolve generated output: {error}"),
        )),
    }
}

fn check_prohibited_host_paths(root: &Path, diagnostics: &mut Vec<Diagnostic>) {
    for prohibited in PROHIBITED_ROOT_PATHS {
        let path = root.join(prohibited);
        if fs::symlink_metadata(&path).is_ok() {
            diagnostics.push(Diagnostic::error("RUSTY_PROHIBITED_HOST_PATH", *prohibited, "this foundation Product Layout does not admit editable host source, HTML, wrapper source, or host build configuration"));
        }
    }
    match fs::read_dir(root) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) if matches_host_config_name(&entry.file_name().to_string_lossy()) => diagnostics.push(Diagnostic::error(
                        "RUSTY_PROHIBITED_HOST_PATH",
                        relative_display(root, &entry.path()),
                        "this foundation Product Layout does not admit editable host source, HTML, wrapper source, or host build configuration",
                    )),
                    Ok(_) => {}
                    Err(error) => diagnostics.push(Diagnostic::error(
                        "RUSTY_DIRECTORY_READ",
                        "$",
                        format!("cannot inspect root host path: {error}"),
                    )),
                }
            }
        }
        Err(error) => diagnostics.push(Diagnostic::error(
            "RUSTY_DIRECTORY_READ",
            "$",
            format!("cannot inspect root host paths: {error}"),
        )),
    }
    for lane in ["rules", "ui", "kernel"] {
        scan_host_paths(root, &root.join(lane), diagnostics);
    }
}

fn scan_host_paths(root: &Path, start: &Path, diagnostics: &mut Vec<Diagnostic>) {
    if !start.exists() {
        return;
    }
    let mut pending = vec![start.to_path_buf()];
    let mut count = 0usize;
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                diagnostics.push(Diagnostic::error(
                    "RUSTY_DIRECTORY_READ",
                    relative_display(root, &directory),
                    format!("cannot inspect product source lane: {error}"),
                ));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    diagnostics.push(Diagnostic::error(
                        "RUSTY_DIRECTORY_READ",
                        relative_display(root, &directory),
                        format!("cannot inspect source lane entry: {error}"),
                    ));
                    continue;
                }
            };
            count += 1;
            if count > MAX_HOST_SCAN_ENTRIES {
                diagnostics.push(Diagnostic::error(
                    "RUSTY_HOST_SCAN_LIMIT",
                    relative_display(root, start),
                    format!("host-path scan is limited to {MAX_HOST_SCAN_ENTRIES} entries"),
                ));
                return;
            }
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    diagnostics.push(Diagnostic::error(
                        "RUSTY_DIRECTORY_READ",
                        relative_display(root, &path),
                        format!("cannot inspect source lane entry: {error}"),
                    ));
                    continue;
                }
            };
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if matches_host_config_name(&entry.file_name().to_string_lossy()) {
                diagnostics.push(Diagnostic::error("RUSTY_PROHIBITED_HOST_PATH", relative_display(root, &path), "this foundation Product Layout does not admit editable host source, HTML, wrapper source, or host build configuration"));
            }
        }
    }
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| "$".to_string())
}

fn matches_host_config_name(name: &str) -> bool {
    name.ends_with(".html")
        || name.starts_with("vite.config.")
        || name.starts_with("webpack.config.")
        || name.starts_with("electron.")
        || name.starts_with("tauri.")
}

fn failure_execution(exit_code: i32, diagnostic: Diagnostic) -> Execution {
    Execution {
        report: Report::failure("error", diagnostic),
        exit_code,
    }
}
