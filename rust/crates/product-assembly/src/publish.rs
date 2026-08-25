use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use cap_primitives::fs::FollowSymlinks;
use cap_std::fs::OpenOptions;
use product_model::ProductPath;

use crate::{
    error::ProductAssemblyError,
    filesystem::{
        checked_product_root, open_dir_nofollow, read_product_file, read_product_tree, ProductRoot,
    },
    receipt::sha256_hex,
    MAX_ASSEMBLY_FILES, MAX_ASSEMBLY_FILE_BYTES, MAX_ASSEMBLY_TOTAL_BYTES,
};

const MAX_PUBLICATION_OUTPUTS: usize = 8;
const STAGE_PREFIX: &str = ".rusty-assembly-stage-";

/// Whether a staged publication destination is one file or one complete
/// directory tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationOutputKind {
    File,
    Directory,
}

/// One product-relative file in a directory publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationFile {
    relative_path: ProductPath,
    bytes: Vec<u8>,
}

impl PublicationFile {
    pub fn new(
        relative_path: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<Self, ProductAssemblyError> {
        let relative_path = ProductPath::parse(relative_path.into()).map_err(|error| {
            ProductAssemblyError::new(
                "ASSEMBLY_PUBLICATION_PATH",
                "publication",
                error.to_string(),
            )
        })?;
        if bytes.len() > MAX_ASSEMBLY_FILE_BYTES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_FILE_BYTES_BOUNDS",
                relative_path.as_str(),
                format!("one publication file is limited to {MAX_ASSEMBLY_FILE_BYTES} bytes"),
            ));
        }
        Ok(Self {
            relative_path,
            bytes,
        })
    }

    pub fn relative_path(&self) -> &ProductPath {
        &self.relative_path
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// One output root in an all-output publication transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationOutput {
    destination: ProductPath,
    kind: PublicationOutputKind,
    file: Option<Vec<u8>>,
    files: Vec<PublicationFile>,
}

impl PublicationOutput {
    pub fn file(
        destination: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<Self, ProductAssemblyError> {
        let destination = ProductPath::parse(destination.into()).map_err(|error| {
            ProductAssemblyError::new(
                "ASSEMBLY_PUBLICATION_PATH",
                "publication",
                error.to_string(),
            )
        })?;
        if bytes.len() > MAX_ASSEMBLY_FILE_BYTES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_FILE_BYTES_BOUNDS",
                destination.as_str(),
                format!("one publication file is limited to {MAX_ASSEMBLY_FILE_BYTES} bytes"),
            ));
        }
        Ok(Self {
            destination,
            kind: PublicationOutputKind::File,
            file: Some(bytes),
            files: Vec::new(),
        })
    }

    pub fn directory(
        destination: impl Into<String>,
        files: Vec<PublicationFile>,
    ) -> Result<Self, ProductAssemblyError> {
        let destination = ProductPath::parse(destination.into()).map_err(|error| {
            ProductAssemblyError::new(
                "ASSEMBLY_PUBLICATION_PATH",
                "publication",
                error.to_string(),
            )
        })?;
        validate_directory_files(destination.as_str(), &files)?;
        Ok(Self {
            destination,
            kind: PublicationOutputKind::Directory,
            file: None,
            files,
        })
    }

    pub fn destination(&self) -> &ProductPath {
        &self.destination
    }

    pub const fn kind(&self) -> PublicationOutputKind {
        self.kind
    }

    pub fn files(&self) -> &[PublicationFile] {
        &self.files
    }

    pub fn file_bytes(&self) -> Option<&[u8]> {
        self.file.as_deref()
    }
}

/// The staged output set returned by an assembly planner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyPublication {
    outputs: Vec<PublicationOutput>,
}

impl AssemblyPublication {
    pub fn new(outputs: Vec<PublicationOutput>) -> Result<Self, ProductAssemblyError> {
        validate_outputs(&outputs)?;
        Ok(Self { outputs })
    }

    pub fn outputs(&self) -> &[PublicationOutput] {
        &self.outputs
    }
}

/// Readback receipt for one successful all-output publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedOutputs {
    outputs: Vec<PublishedOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedOutput {
    destination: String,
    kind: PublicationOutputKind,
    entries: Vec<PublishedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedFile {
    path: String,
    bytes: usize,
    sha256: String,
}

impl PublishedOutputs {
    pub fn outputs(&self) -> &[PublishedOutput] {
        &self.outputs
    }
}

impl PublishedOutput {
    pub fn destination(&self) -> &str {
        &self.destination
    }

    pub const fn kind(&self) -> PublicationOutputKind {
        self.kind
    }

    pub fn entries(&self) -> &[PublishedFile] {
        &self.entries
    }
}

impl PublishedFile {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn byte_length(&self) -> usize {
        self.bytes
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Stages, verifies, and atomically swaps the complete Engine-owned
/// `generated/` tree. Existing generated output is moved to a recoverable
/// sibling backup before the staged tree is installed; no observer can see a
/// partially populated set of the four Product Assembly lanes.
pub fn publish_outputs(
    product_root: &Path,
    publication: &AssemblyPublication,
) -> Result<PublishedOutputs, ProductAssemblyError> {
    publish_outputs_inner(product_root, publication, None)
}

fn publish_outputs_inner(
    product_root: &Path,
    publication: &AssemblyPublication,
    fail_after_swap: Option<bool>,
) -> Result<PublishedOutputs, ProductAssemblyError> {
    validate_outputs(&publication.outputs)?;
    let root = checked_product_root(product_root)?;
    let root_dir = root.directory();
    validate_existing_generated(root_dir)?;

    let (stage_name, stage_dir, backup_name) = create_stage(root_dir)?;
    let stage_generated = match stage_dir.create_dir("generated") {
        Ok(()) => open_dir_nofollow(&stage_dir, "generated"),
        Err(error) => Err(error),
    }
    .map_err(|error| {
        let cleanup = remove_relative_tree(root_dir, &stage_name);
        publication_error_with_cleanup(
            ProductAssemblyError::io("ASSEMBLY_PUBLICATION_STAGE", "generated", error),
            &stage_name,
            cleanup,
        )
    })?;

    if let Err(error) = stage_outputs(&stage_generated, publication) {
        let cleanup = remove_relative_tree(root_dir, &stage_name);
        return Err(publication_error_with_cleanup(error, &stage_name, cleanup));
    }
    let stage_root = ProductRoot::from_directory(
        stage_dir.try_clone().map_err(|error| {
            ProductAssemblyError::io("ASSEMBLY_PUBLICATION_STAGE", "generated", error)
        })?,
        root.path().join(&stage_name),
    );
    if let Err(error) = readback_published(&stage_root, publication) {
        let cleanup = remove_relative_tree(root_dir, &stage_name);
        return Err(publication_error_with_cleanup(error, &stage_name, cleanup));
    }

    let had_existing = match root_dir.symlink_metadata("generated") {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => {
            let cleanup = remove_relative_tree(root_dir, &stage_name);
            return Err(publication_error_with_cleanup(
                ProductAssemblyError::io("ASSEMBLY_GENERATED_READ", "generated", error),
                &stage_name,
                cleanup,
            ));
        }
    };
    if had_existing {
        if let Err(error) = root_dir.rename("generated", root_dir, &backup_name) {
            let cleanup = remove_relative_tree(root_dir, &stage_name);
            return Err(publication_error_with_cleanup(
                ProductAssemblyError::io("ASSEMBLY_PUBLICATION_BACKUP", "generated", error),
                &stage_name,
                cleanup,
            ));
        }
    }

    let swap_result = stage_dir
        .rename("generated", root_dir, "generated")
        .map_err(|error| ProductAssemblyError::io("ASSEMBLY_PUBLICATION_SWAP", "generated", error));
    if let Err(error) = swap_result {
        let rollback = rollback_tree(root_dir, &stage_name, &backup_name, had_existing, false);
        return Err(with_rollback(error, rollback));
    }
    if fail_after_swap == Some(true) {
        let error = ProductAssemblyError::new(
            "ASSEMBLY_PUBLICATION_INJECTED_FAILURE",
            "generated",
            "injected post-swap failure used to prove complete-tree rollback",
        );
        let rollback = rollback_tree(root_dir, &stage_name, &backup_name, had_existing, true);
        return Err(with_rollback(error, rollback));
    }

    let result = readback_published(&root, publication);
    if let Err(error) = result {
        let rollback = rollback_tree(root_dir, &stage_name, &backup_name, had_existing, true);
        return Err(with_rollback(error, rollback));
    }

    if had_existing {
        if let Err(error) = root_dir.remove_dir_all(&backup_name) {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_PUBLICATION_CLEANUP",
                &backup_name,
                format!(
                    "{error}; recoverable backup retained at `{backup_name}` and stage `{stage_name}` remains for manual recovery"
                ),
            ));
        }
    }
    if let Err(error) = root_dir.remove_dir_all(&stage_name) {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_PUBLICATION_CLEANUP",
            &stage_name,
            format!("{error}; published generated tree is valid; stage retained for recovery"),
        ));
    }
    result
}

/// Verifies an already-published output set without changing the product
/// root. This is intentionally the same exact readback used after a swap.
pub fn verify_outputs(
    product_root: &Path,
    publication: &AssemblyPublication,
) -> Result<PublishedOutputs, ProductAssemblyError> {
    validate_outputs(&publication.outputs)?;
    let root = checked_product_root(product_root)?;
    readback_published(&root, publication)
}

#[cfg(test)]
pub(crate) fn publish_outputs_fail_after_swap(
    product_root: &Path,
    publication: &AssemblyPublication,
) -> Result<PublishedOutputs, ProductAssemblyError> {
    publish_outputs_inner(product_root, publication, Some(true))
}

fn validate_outputs(outputs: &[PublicationOutput]) -> Result<(), ProductAssemblyError> {
    if outputs.is_empty() || outputs.len() > MAX_PUBLICATION_OUTPUTS {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_OUTPUT_COUNT_BOUNDS",
            "publication",
            format!("publication must contain 1..={MAX_PUBLICATION_OUTPUTS} outputs"),
        ));
    }
    let mut destinations = BTreeSet::new();
    let mut total_bytes = 0usize;
    let mut total_files = 0usize;
    for output in outputs {
        let destination = output.destination.as_str();
        if !destinations.insert(destination) {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_DUPLICATE_OUTPUT",
                destination,
                "publication destinations must be unique",
            ));
        }
        if !destination.starts_with("generated/") {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_OUTPUT_LANE",
                destination,
                "Product Assembly outputs must remain below the generated lane",
            ));
        }
        for known in destinations.iter().filter(|known| **known != destination) {
            if within(known, destination) || within(destination, known) {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_OUTPUT_OVERLAP",
                    destination,
                    "publication destinations must not overlap",
                ));
            }
        }
        match output.kind {
            PublicationOutputKind::File => {
                total_files = total_files.checked_add(1).ok_or_else(|| {
                    ProductAssemblyError::new(
                        "ASSEMBLY_FILE_COUNT_BOUNDS",
                        "publication",
                        "file count overflowed",
                    )
                })?;
                total_bytes = total_bytes
                    .checked_add(output.file.as_ref().map_or(0, Vec::len))
                    .ok_or_else(|| {
                        ProductAssemblyError::new(
                            "ASSEMBLY_TOTAL_BYTES_BOUNDS",
                            "publication",
                            "byte count overflowed",
                        )
                    })?;
            }
            PublicationOutputKind::Directory => {
                validate_directory_files(destination, &output.files)?;
                total_files = total_files.checked_add(output.files.len()).ok_or_else(|| {
                    ProductAssemblyError::new(
                        "ASSEMBLY_FILE_COUNT_BOUNDS",
                        "publication",
                        "file count overflowed",
                    )
                })?;
                for file in &output.files {
                    total_bytes = total_bytes.checked_add(file.bytes.len()).ok_or_else(|| {
                        ProductAssemblyError::new(
                            "ASSEMBLY_TOTAL_BYTES_BOUNDS",
                            "publication",
                            "byte count overflowed",
                        )
                    })?;
                }
            }
        }
        if total_files > MAX_ASSEMBLY_FILES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_FILE_COUNT_BOUNDS",
                "publication",
                format!("publication is limited to {MAX_ASSEMBLY_FILES} files"),
            ));
        }
        if total_bytes > MAX_ASSEMBLY_TOTAL_BYTES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_TOTAL_BYTES_BOUNDS",
                "publication",
                format!("publication is limited to {MAX_ASSEMBLY_TOTAL_BYTES} bytes"),
            ));
        }
    }
    Ok(())
}

fn validate_directory_files(
    _destination: &str,
    files: &[PublicationFile],
) -> Result<(), ProductAssemblyError> {
    let mut paths = BTreeSet::new();
    for file in files {
        if !paths.insert(file.relative_path.as_str()) {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_DUPLICATE_PUBLICATION_FILE",
                file.relative_path.as_str(),
                "directory publication files must be unique",
            ));
        }
    }
    Ok(())
}

fn within(child: &str, parent: &str) -> bool {
    child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn validate_existing_generated(root: &cap_std::fs::Dir) -> Result<(), ProductAssemblyError> {
    match root.symlink_metadata("generated") {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(ProductAssemblyError::new(
            "ASSEMBLY_GENERATED_SYMLINK",
            "generated",
            "the generated lane must not be a symlink",
        )),
        Ok(metadata) if !metadata.is_dir() => Err(ProductAssemblyError::new(
            "ASSEMBLY_GENERATED_DIRECTORY",
            "generated",
            "the generated lane must be a directory",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ProductAssemblyError::io(
            "ASSEMBLY_GENERATED_READ",
            "generated",
            error,
        )),
    }
}

fn create_stage(
    root: &cap_std::fs::Dir,
) -> Result<(String, cap_std::fs::Dir, String), ProductAssemblyError> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            ProductAssemblyError::new("ASSEMBLY_PUBLICATION_STAGE", "$", error.to_string())
        })?
        .as_nanos();
    for attempt in 0..16u32 {
        let stage_name = format!("{STAGE_PREFIX}{nonce}-{attempt}");
        match root.create_dir(&stage_name) {
            Ok(()) => {
                let backup_name = format!("{stage_name}-backup");
                if root.symlink_metadata(&backup_name).is_ok() {
                    let _ = root.remove_dir_all(&stage_name);
                    continue;
                }
                let stage_dir = open_dir_nofollow(root, &stage_name).map_err(|error| {
                    let _ = root.remove_dir_all(&stage_name);
                    ProductAssemblyError::io("ASSEMBLY_PUBLICATION_STAGE", &stage_name, error)
                })?;
                return Ok((stage_name, stage_dir, backup_name));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(ProductAssemblyError::io(
                    "ASSEMBLY_PUBLICATION_STAGE",
                    "generated",
                    error,
                ))
            }
        }
    }
    Err(ProductAssemblyError::new(
        "ASSEMBLY_PUBLICATION_STAGE",
        "generated",
        "could not allocate an exclusive publication stage",
    ))
}

fn stage_outputs(
    stage_generated: &cap_std::fs::Dir,
    publication: &AssemblyPublication,
) -> Result<(), ProductAssemblyError> {
    for output in &publication.outputs {
        let destination = output
            .destination
            .as_str()
            .strip_prefix("generated/")
            .expect("validated generated destination");
        match output.kind {
            PublicationOutputKind::File => {
                write_staged_file(
                    stage_generated,
                    destination,
                    output.file.as_ref().expect("file output has bytes"),
                )?;
            }
            PublicationOutputKind::Directory => {
                ensure_staged_directory(stage_generated, destination)?;
                for file in &output.files {
                    write_staged_file(
                        stage_generated,
                        &format!("{destination}/{}", file.relative_path.as_str()),
                        &file.bytes,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn ensure_staged_directory(
    root: &cap_std::fs::Dir,
    relative_path: &str,
) -> Result<(), ProductAssemblyError> {
    let mut directory = root.try_clone().map_err(|error| {
        ProductAssemblyError::io("ASSEMBLY_PUBLICATION_STAGE_WRITE", relative_path, error)
    })?;
    for component in relative_path.split('/') {
        directory = match open_dir_nofollow(&directory, component) {
            Ok(directory) => directory,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                directory.create_dir(component).map_err(|error| {
                    ProductAssemblyError::io(
                        "ASSEMBLY_PUBLICATION_STAGE_WRITE",
                        relative_path,
                        error,
                    )
                })?;
                open_dir_nofollow(&directory, component).map_err(|error| {
                    ProductAssemblyError::io(
                        "ASSEMBLY_PUBLICATION_STAGE_WRITE",
                        relative_path,
                        error,
                    )
                })?
            }
            Err(error) => {
                return Err(ProductAssemblyError::io(
                    "ASSEMBLY_PUBLICATION_STAGE_WRITE",
                    relative_path,
                    error,
                ))
            }
        };
    }
    Ok(())
}

fn write_staged_file(
    root: &cap_std::fs::Dir,
    relative_path: &str,
    bytes: &[u8],
) -> Result<(), ProductAssemblyError> {
    let components = relative_path.split('/').collect::<Vec<_>>();
    let (name, parents) = components
        .split_last()
        .expect("ProductPath destination has one component");
    let mut directory = root.try_clone().map_err(|error| {
        ProductAssemblyError::io("ASSEMBLY_PUBLICATION_STAGE_WRITE", relative_path, error)
    })?;
    for parent in parents {
        directory = match open_dir_nofollow(&directory, parent) {
            Ok(directory) => directory,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                directory.create_dir(parent).map_err(|error| {
                    ProductAssemblyError::io(
                        "ASSEMBLY_PUBLICATION_STAGE_WRITE",
                        relative_path,
                        error,
                    )
                })?;
                open_dir_nofollow(&directory, parent).map_err(|error| {
                    ProductAssemblyError::io(
                        "ASSEMBLY_PUBLICATION_STAGE_WRITE",
                        relative_path,
                        error,
                    )
                })?
            }
            Err(error) => {
                return Err(ProductAssemblyError::io(
                    "ASSEMBLY_PUBLICATION_STAGE_WRITE",
                    relative_path,
                    error,
                ))
            }
        };
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    options._cap_fs_ext_follow(FollowSymlinks::No);
    let mut file = directory.open_with(name, &options).map_err(|error| {
        ProductAssemblyError::io("ASSEMBLY_PUBLICATION_STAGE_WRITE", relative_path, error)
    })?;
    file.write_all(bytes).map_err(|error| {
        ProductAssemblyError::io("ASSEMBLY_PUBLICATION_STAGE_WRITE", relative_path, error)
    })
}

fn readback_published(
    root: &ProductRoot,
    publication: &AssemblyPublication,
) -> Result<PublishedOutputs, ProductAssemblyError> {
    let mut outputs = Vec::with_capacity(publication.outputs.len());
    for output in &publication.outputs {
        let mut entries = Vec::new();
        match output.kind {
            PublicationOutputKind::File => {
                let bytes =
                    read_product_file(root, &output.destination, output.destination.as_str())?
                        .bytes;
                let expected = output.file.as_ref().expect("file output has bytes");
                if bytes != *expected {
                    return Err(ProductAssemblyError::new(
                        "ASSEMBLY_PUBLICATION_READBACK",
                        output.destination.as_str(),
                        "published file failed exact readback",
                    ));
                }
                entries.push(PublishedFile {
                    path: output.destination.as_str().to_owned(),
                    bytes: bytes.len(),
                    sha256: sha256_hex(&bytes),
                });
            }
            PublicationOutputKind::Directory => {
                let actual =
                    read_product_tree(root, &output.destination, output.destination.as_str())?;
                let prefix = format!("{}/", output.destination.as_str());
                let actual_by_path = actual
                    .iter()
                    .map(|file| {
                        (
                            file.relative_path
                                .as_str()
                                .strip_prefix(&prefix)
                                .unwrap_or(file.relative_path.as_str()),
                            file,
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                if actual_by_path.len() != output.files.len() {
                    return Err(ProductAssemblyError::new(
                        "ASSEMBLY_PUBLICATION_READBACK",
                        output.destination.as_str(),
                        "published directory contains an extra or missing file",
                    ));
                }
                for file in &output.files {
                    let Some(actual_file) = actual_by_path.get(file.relative_path.as_str()) else {
                        return Err(ProductAssemblyError::new(
                            "ASSEMBLY_PUBLICATION_READBACK",
                            file.relative_path.as_str(),
                            "published directory is missing an expected file",
                        ));
                    };
                    if actual_file.bytes != file.bytes {
                        return Err(ProductAssemblyError::new(
                            "ASSEMBLY_PUBLICATION_READBACK",
                            file.relative_path.as_str(),
                            "published file failed exact readback",
                        ));
                    }
                    entries.push(PublishedFile {
                        path: format!("{}/{}", output.destination, file.relative_path.as_str()),
                        bytes: actual_file.bytes.len(),
                        sha256: sha256_hex(&actual_file.bytes),
                    });
                }
            }
        }
        outputs.push(PublishedOutput {
            destination: output.destination.as_str().to_owned(),
            kind: output.kind,
            entries,
        });
    }
    Ok(PublishedOutputs { outputs })
}

fn rollback_tree(
    root: &cap_std::fs::Dir,
    stage_name: &str,
    backup_name: &str,
    had_existing: bool,
    published: bool,
) -> Result<(), String> {
    let mut failures = Vec::new();
    if published {
        if let Err(error) = root.remove_dir_all("generated") {
            if error.kind() != io::ErrorKind::NotFound {
                failures.push(format!("generated: {error}"));
            }
        }
    }
    let mut restored = true;
    if had_existing {
        if let Err(error) = root.rename(backup_name, root, "generated") {
            restored = false;
            failures.push(format!("{backup_name} -> generated: {error}"));
        }
    }
    // Keep the stage alongside any unrecovered backup so diagnostics identify
    // a recoverable state rather than deleting the only prior output.
    if restored {
        if let Err(error) = root.remove_dir_all(stage_name) {
            if error.kind() != io::ErrorKind::NotFound {
                failures.push(format!("{stage_name}: {error}"));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn remove_relative_tree(root: &cap_std::fs::Dir, path: &str) -> io::Result<()> {
    match root.symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => root.remove_file(path),
        Ok(metadata) if metadata.is_dir() => root.remove_dir_all(path),
        Ok(_) => root.remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn publication_error_with_cleanup(
    error: ProductAssemblyError,
    stage_name: &str,
    cleanup: io::Result<()>,
) -> ProductAssemblyError {
    match cleanup {
        Ok(()) => error,
        Err(cleanup_error) => ProductAssemblyError::new(
            "ASSEMBLY_PUBLICATION_CLEANUP",
            stage_name,
            format!(
                "{}; stage retained for recovery: {cleanup_error}",
                error.diagnostic().message()
            ),
        ),
    }
}

fn with_rollback(
    error: ProductAssemblyError,
    rollback: Result<(), String>,
) -> ProductAssemblyError {
    match rollback {
        Ok(()) => error,
        Err(details) => ProductAssemblyError::new(
            "ASSEMBLY_PUBLICATION_ROLLBACK",
            error.diagnostic().path(),
            format!(
                "{}; rollback failed: {details}",
                error.diagnostic().message()
            ),
        ),
    }
}
