use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};

use product_model::decode_product_manifest;

use crate::{
    report::{Diagnostic, Report},
    Execution, EXIT_CONFORMANCE, EXIT_ROOT, EXIT_USAGE,
};

const MANIFEST_NAME: &str = "rusty.toml";
const GENERATED_LANE: &str = "generated";
const MINIMUM_MANIFEST_PREFIX: &str = "[product]\nid = \"";
const INIT_TARGET_PATH: &str = "$target";
const INIT_PARENT_PATH: &str = "$parent";
const MAX_INIT_PARENT_COMPONENTS: usize = 64;

pub(crate) fn init(target: PathBuf, product_id: Option<String>) -> Execution {
    if let Err(diagnostic) = validate_parent_chain(&target) {
        return failure_execution(EXIT_CONFORMANCE, diagnostic);
    }
    let product_id =
        match product_id.or_else(|| inferred_product_id(&target)) {
            Some(product_id) => product_id,
            None => return failure_execution(
                EXIT_USAGE,
                Diagnostic::error(
                    "RUSTY_INIT_PRODUCT_ID_REQUIRED",
                    INIT_TARGET_PATH,
                    "supply --id <lowercase-product-id> when the target directory cannot form one",
                ),
            ),
        };
    let minimum = match MinimumProduct::new(product_id) {
        Ok(minimum) => minimum,
        Err(diagnostic) => return failure_execution(EXIT_USAGE, diagnostic),
    };

    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() => failure_execution(
            EXIT_CONFORMANCE,
            Diagnostic::error(
                "RUSTY_INIT_TARGET_SYMLINK",
                INIT_TARGET_PATH,
                "init target must not be a symlink; choose a new product root",
            ),
        ),
        Ok(metadata) if metadata.is_dir() => init_existing(&target, &minimum),
        Ok(_) => failure_execution(
            EXIT_CONFORMANCE,
            Diagnostic::error(
                "RUSTY_INIT_TARGET_NOT_DIRECTORY",
                INIT_TARGET_PATH,
                "init target exists and is not a directory",
            ),
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => init_new(&target, &minimum),
        Err(error) => failure_execution(
            EXIT_ROOT,
            Diagnostic::error(
                "RUSTY_INIT_TARGET_READ",
                INIT_TARGET_PATH,
                format!("cannot inspect init target: {error}"),
            ),
        ),
    }
}

fn inferred_product_id(target: &Path) -> Option<String> {
    let name = target.file_name()?.to_str()?;
    let mut id = String::from("rusty.");
    let mut previous_separator = false;
    for character in name.chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            id.push(character);
            previous_separator = false;
        } else if !previous_separator {
            id.push('-');
            previous_separator = true;
        }
    }
    if id.ends_with('-') {
        id.pop();
    }
    (id != "rusty.").then_some(id)
}

struct MinimumProduct {
    files: Vec<(&'static str, Vec<u8>)>,
    directories: Vec<&'static str>,
}

impl MinimumProduct {
    fn new(product_id: String) -> Result<Self, Diagnostic> {
        let manifest = format!(
            "{MINIMUM_MANIFEST_PREFIX}{product_id}\"\n\n[runtime_composition]\nentrypoints = [\"rules/main.ts\"]\n\n[lifecycle]\nmode = \"demand\"\n\n[ui]\nentry = \"ui/main.ts\"\n\n[content]\nroot = \"content\"\n\n[outputs]\ncompiled_composition = \"generated/compiled-composition.json\"\nadmitted_runtime_content = \"generated/runtime-content\"\nproduct_assembly = \"generated/product-assembly\"\nproduct_bundle = \"generated/product-bundle\"\n"
        );
        decode_product_manifest(&manifest).map_err(|error| {
            Diagnostic::error(
                "RUSTY_INIT_PRODUCT_ID_INVALID",
                "--id",
                error.diagnostic().message(),
            )
        })?;
        let rules = format!(
            "import {{ schedule }} from '@rusty-engine/runtime-composition-authoring';\n\nexport default {{\n  product: '{product_id}',\n  capabilities: [],\n  intentDescriptors: [],\n  inputMap: [],\n  schedule: schedule({{}}),\n  gameplayDefinitions: [],\n  timelines: [],\n}};\n"
        );
        let ui = "import type { RustyApplicationUiOwner } from '@rusty-engine/application-host';\n\nexport function mountProductUi(root: HTMLElement): RustyApplicationUiOwner {\n  const label = document.createElement('p');\n  label.textContent = 'Rusty product ready';\n  root.append(label);\n  return { dispose: () => label.remove() };\n}\n";
        Ok(Self {
            files: vec![
                (MANIFEST_NAME, manifest.into_bytes()),
                ("rules/main.ts", rules.into_bytes()),
                ("ui/main.ts", ui.as_bytes().to_vec()),
                ("content/.keep", Vec::new()),
                ("generated/.keep", Vec::new()),
            ],
            directories: vec!["rules", "ui", "content", GENERATED_LANE],
        })
    }

    fn expected_paths(&self) -> BTreeSet<String> {
        self.files
            .iter()
            .map(|(path, _)| (*path).to_string())
            .chain(self.directories.iter().map(|path| (*path).to_string()))
            .collect()
    }
}

fn init_existing(target: &Path, minimum: &MinimumProduct) -> Execution {
    let entries = match collect_product_paths(target) {
        Ok(entries) => entries,
        Err(diagnostic) => return failure_execution(EXIT_ROOT, diagnostic),
    };
    if entries.is_empty() {
        return init_empty_existing(target, minimum);
    }
    if entries != minimum.expected_paths() {
        return failure_execution(
            EXIT_CONFORMANCE,
            Diagnostic::error(
                "RUSTY_INIT_CONFLICT",
                INIT_TARGET_PATH,
                "init never overwrites an existing product root unless it is the exact generated minimum",
            ),
        );
    }
    for (relative, expected) in &minimum.files {
        let path = target.join(relative);
        if !matches!(fs::symlink_metadata(&path), Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink())
        {
            return failure_execution(
                EXIT_CONFORMANCE,
                Diagnostic::error(
                    "RUSTY_INIT_CONFLICT",
                    *relative,
                    "expected generated file must be a regular non-symlink file",
                ),
            );
        }
        match fs::read(&path) {
            Ok(actual) if actual == *expected => {}
            Ok(_) | Err(_) => {
                return failure_execution(
                    EXIT_CONFORMANCE,
                    Diagnostic::error(
                        "RUSTY_INIT_CONFLICT",
                        *relative,
                        "existing product does not match the exact generated minimum; no files were changed",
                    ),
                )
            }
        }
    }
    for directory in &minimum.directories {
        if !matches!(fs::symlink_metadata(target.join(directory)), Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink())
        {
            return failure_execution(
                EXIT_CONFORMANCE,
                Diagnostic::error(
                    "RUSTY_INIT_CONFLICT",
                    *directory,
                    "existing product does not match the exact generated minimum; no files were changed",
                ),
            );
        }
    }
    Execution {
        report: Report::success(),
        exit_code: 0,
    }
}

fn init_empty_existing(target: &Path, minimum: &MinimumProduct) -> Execution {
    match open_product_dir(target) {
        Ok(root) => publish_minimum(&root, minimum),
        Err(error) => failure_execution(
            EXIT_ROOT,
            Diagnostic::error(
                "RUSTY_INIT_TARGET_READ",
                INIT_TARGET_PATH,
                format!("cannot open init target without following path aliases: {error}"),
            ),
        ),
    }
}

fn init_new(target: &Path, minimum: &MinimumProduct) -> Execution {
    let parent = match target.parent() {
        Some(parent) => parent,
        None => {
            return failure_execution(
                EXIT_USAGE,
                Diagnostic::error(
                    "RUSTY_INIT_TARGET_INVALID",
                    INIT_TARGET_PATH,
                    "init target must have a parent directory",
                ),
            )
        }
    };
    if !parent.is_dir() {
        return failure_execution(
            EXIT_ROOT,
            Diagnostic::error(
                "RUSTY_INIT_PARENT_MISSING",
                INIT_PARENT_PATH,
                "init parent directory must already exist",
            ),
        );
    }
    // The capability directory pins publication below this parent. Every
    // product path below is created relative to that handle with no-replace
    // operations, so a concurrent creator cannot be overwritten or redirect
    // an intermediate lane through a pathname alias.
    let parent_dir = match Dir::open_ambient_dir(parent, ambient_authority()) {
        Ok(parent_dir) => parent_dir,
        Err(error) => {
            return failure_execution(
                EXIT_ROOT,
                Diagnostic::error(
                    "RUSTY_INIT_PARENT_MISSING",
                    INIT_PARENT_PATH,
                    format!("cannot open init parent: {error}"),
                ),
            )
        }
    };
    let name = match target.file_name() {
        Some(name) => name,
        None => {
            return failure_execution(
                EXIT_USAGE,
                Diagnostic::error(
                    "RUSTY_INIT_TARGET_INVALID",
                    INIT_TARGET_PATH,
                    "init target must have one final directory name",
                ),
            )
        }
    };
    if let Err(error) = parent_dir.create_dir(name) {
        return failure_execution(
            EXIT_CONFORMANCE,
            Diagnostic::error(
                "RUSTY_INIT_CONFLICT",
                INIT_TARGET_PATH,
                format!("init target could not be created exclusively: {error}"),
            ),
        );
    }
    let root = match parent_dir.open_dir(name) {
        Ok(root) => root,
        Err(error) => {
            return append_target_cleanup(
                failure_execution(
                    EXIT_ROOT,
                    Diagnostic::error(
                        "RUSTY_INIT_TARGET_READ",
                        INIT_TARGET_PATH,
                        format!("exclusive init target could not be opened: {error}"),
                    ),
                ),
                &parent_dir,
                name,
            )
        }
    };
    let result = publish_minimum(&root, minimum);
    if result.exit_code != 0 {
        return append_target_cleanup(result, &parent_dir, name);
    }
    result
}

fn append_target_cleanup(mut result: Execution, parent: &Dir, name: &std::ffi::OsStr) -> Execution {
    if let Err(error) = parent.remove_dir(name) {
        if let Some(diagnostic) = result.report.diagnostics.first_mut() {
            diagnostic
                .message
                .push_str(&format!("; RUSTY_INIT_TARGET_ROLLBACK failed: {error}"));
        }
    }
    result
}

fn open_product_dir(target: &Path) -> io::Result<Dir> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    let name = target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no final name"))?;
    let parent = Dir::open_ambient_dir(parent, ambient_authority())?;
    let metadata = parent.symlink_metadata(name)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "target is a symlink",
        ));
    }
    parent.open_dir(name)
}

fn publish_minimum(root: &Dir, minimum: &MinimumProduct) -> Execution {
    let mut directories = Vec::new();
    let mut files = Vec::new();
    for directory in &minimum.directories {
        if let Err(error) = root.create_dir(directory) {
            let rollback = rollback_cap(root, &files, &directories);
            return failure_execution(EXIT_CONFORMANCE, Diagnostic::error("RUSTY_INIT_CONFLICT", *directory, format!("init publishes only through no-replace directory creation: {error}{rollback}")));
        }
        directories.push(*directory);
    }
    for (relative, bytes) in &minimum.files {
        let (parent, name) = relative.rsplit_once('/').unwrap_or((".", relative));
        let directory = match root.open_dir(parent) {
            Ok(directory) => directory,
            Err(error) => {
                let rollback = rollback_cap(root, &files, &directories);
                return failure_execution(
                    EXIT_ROOT,
                    Diagnostic::error(
                        "RUSTY_INIT_PUBLISH",
                        *relative,
                        format!("cannot open precreated lane: {error}{rollback}"),
                    ),
                );
            }
        };
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = match directory.open_with(name, &options) {
            Ok(file) => {
                files.push(*relative);
                file
            }
            Err(error) => {
                let rollback = rollback_cap(root, &files, &directories);
                return failure_execution(EXIT_CONFORMANCE, Diagnostic::error("RUSTY_INIT_CONFLICT", *relative, format!("init publishes only through no-replace file creation: {error}{rollback}")));
            }
        };
        let write = std::io::Write::write_all(&mut file, bytes);
        drop(file);
        if let Err(error) = write {
            let rollback = rollback_cap(root, &files, &directories);
            return failure_execution(EXIT_ROOT, Diagnostic::error("RUSTY_INIT_PUBLISH", *relative, format!("created file could not be fully written and was rolled back: {error}{rollback}")));
        }
    }
    Execution {
        report: Report::success(),
        exit_code: 0,
    }
}

fn validate_parent_chain(target: &Path) -> Result<(), Diagnostic> {
    let parent = target.parent().ok_or_else(|| {
        Diagnostic::error(
            "RUSTY_INIT_TARGET_INVALID",
            INIT_TARGET_PATH,
            "init target must have a parent directory",
        )
    })?;
    let mut current = PathBuf::new();
    let mut count = 0usize;
    for component in parent.components() {
        current.push(component.as_os_str());
        count += 1;
        if count > MAX_INIT_PARENT_COMPONENTS {
            return Err(Diagnostic::error("RUSTY_INIT_PARENT_DEPTH", INIT_PARENT_PATH, format!("init parent has more than {MAX_INIT_PARENT_COMPONENTS} lexical path components")));
        }
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            Diagnostic::error(
                "RUSTY_INIT_PARENT_READ",
                INIT_PARENT_PATH,
                format!("cannot inspect lexical init parent component: {error}"),
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(Diagnostic::error(
                "RUSTY_INIT_PARENT_ALIAS",
                INIT_PARENT_PATH,
                "init does not admit symlinked or non-directory lexical parent components",
            ));
        }
    }
    Ok(())
}

fn rollback_cap(root: &Dir, files: &[&str], directories: &[&str]) -> String {
    let mut failures = Vec::new();
    for relative in files.iter().rev() {
        if let Some((parent, name)) = relative.rsplit_once('/') {
            match root.open_dir(parent).and_then(|dir| dir.remove_file(name)) {
                Ok(()) => {}
                Err(error) => failures.push(format!("{relative}: {error}")),
            }
        } else if let Err(error) = root.remove_file(relative) {
            failures.push(format!("{relative}: {error}"));
        }
    }
    for directory in directories.iter().rev() {
        if let Err(error) = root.remove_dir(directory) {
            failures.push(format!("{directory}: {error}"));
        }
    }
    if failures.is_empty() {
        String::new()
    } else {
        format!("; RUSTY_INIT_ROLLBACK failed: {}", failures.join(", "))
    }
}

fn collect_product_paths(path: &Path) -> Result<BTreeSet<String>, Diagnostic> {
    let mut entries = BTreeSet::new();
    let mut pending = vec![path.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| {
            Diagnostic::error(
                "RUSTY_DIRECTORY_READ",
                INIT_TARGET_PATH,
                format!("cannot read directory: {error}"),
            )
        })? {
            let entry = entry.map_err(|error| {
                Diagnostic::error(
                    "RUSTY_DIRECTORY_READ",
                    INIT_TARGET_PATH,
                    format!("cannot read directory entry: {error}"),
                )
            })?;
            let entry_path = entry.path();
            let relative = entry_path
                .strip_prefix(path)
                .expect("descendant entry is relative to its scanned root")
                .to_string_lossy()
                .replace('\\', "/");
            entries.insert(relative);
            if entries.len() > crate::report::MAX_DIAGNOSTICS {
                return Err(Diagnostic::error(
                    "RUSTY_INIT_CONFLICT",
                    INIT_TARGET_PATH,
                    "existing product contains too many paths to be the exact generated minimum",
                ));
            }
            if fs::symlink_metadata(&entry_path)
                .map_err(|error| {
                    Diagnostic::error(
                        "RUSTY_DIRECTORY_READ",
                        INIT_TARGET_PATH,
                        format!("cannot inspect directory entry: {error}"),
                    )
                })?
                .is_dir()
            {
                pending.push(entry_path);
            }
        }
    }
    Ok(entries)
}

fn failure_execution(exit_code: i32, diagnostic: Diagnostic) -> Execution {
    Execution {
        report: Report::failure("error", diagnostic),
        exit_code,
    }
}
