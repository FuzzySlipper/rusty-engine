use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};

use cap_primitives::fs::FollowSymlinks;
use cap_std::{
    ambient_authority,
    fs::{Dir, DirEntry, OpenOptions},
};
use product_model::ProductPath;

use crate::{
    error::ProductAssemblyError, MAX_ASSEMBLY_FILES, MAX_ASSEMBLY_FILE_BYTES,
    MAX_ASSEMBLY_PATH_DEPTH, MAX_ASSEMBLY_TOTAL_BYTES,
};

/// One bounded regular file retained by a Product Assembly plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductFile {
    pub(crate) relative_path: ProductPath,
    pub(crate) bytes: Vec<u8>,
}

/// An opened product root capability. Reads below this handle are relative to
/// the already-admitted directory, so a concurrent path replacement cannot
/// redirect an input read through an ambient absolute path.
pub(crate) struct ProductRoot {
    directory: Dir,
    /// Canonical path is retained for operation diagnostics only. It is never
    /// serialized into a receipt or generated source.
    #[allow(dead_code)]
    canonical_path: PathBuf,
}

impl ProductRoot {
    pub(crate) fn path(&self) -> &Path {
        &self.canonical_path
    }

    pub(crate) fn directory(&self) -> &Dir {
        &self.directory
    }

    pub(crate) fn from_directory(directory: Dir, canonical_path: PathBuf) -> Self {
        Self {
            directory,
            canonical_path,
        }
    }
}

/// Opens the operation root without accepting a symlink at its final
/// component. The returned capability is then used for all source reads.
pub(crate) fn checked_product_root(root: &Path) -> Result<ProductRoot, ProductAssemblyError> {
    let parent = root.parent().ok_or_else(|| {
        ProductAssemblyError::new(
            "ASSEMBLY_PRODUCT_ROOT_READ",
            "$root",
            "the product root must have a parent directory",
        )
    })?;
    let name = root.file_name().ok_or_else(|| {
        ProductAssemblyError::new(
            "ASSEMBLY_PRODUCT_ROOT_READ",
            "$root",
            "the product root must have a final directory name",
        )
    })?;
    let parent_dir = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|error| ProductAssemblyError::io("ASSEMBLY_PRODUCT_ROOT_READ", "$root", error))?;
    let metadata = parent_dir
        .symlink_metadata(name)
        .map_err(|error| ProductAssemblyError::io("ASSEMBLY_PRODUCT_ROOT_READ", "$root", error))?;
    if metadata.file_type().is_symlink() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_PRODUCT_ROOT_SYMLINK",
            "$root",
            "the product root must not be a symlink",
        ));
    }
    if !metadata.is_dir() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_PRODUCT_ROOT_DIRECTORY",
            "$root",
            "the product root must be a directory",
        ));
    }
    let directory = open_dir_nofollow(&parent_dir, name)
        .map_err(|error| ProductAssemblyError::io("ASSEMBLY_PRODUCT_ROOT_READ", "$root", error))?;
    let canonical_path = std::fs::canonicalize(root).map_err(|error| {
        ProductAssemblyError::io("ASSEMBLY_PRODUCT_ROOT_CANONICALIZE", "$root", error)
    })?;
    Ok(ProductRoot {
        directory,
        canonical_path,
    })
}

pub(crate) fn read_product_file(
    root: &ProductRoot,
    relative_path: &ProductPath,
    logical_path: &str,
) -> Result<ProductFile, ProductAssemblyError> {
    let components = checked_components(relative_path, logical_path)?;
    let (name, parent_components) = components
        .split_last()
        .expect("ProductPath always has one component");
    let parent = open_relative_directory(&root.directory, parent_components, logical_path)?;
    let metadata = parent
        .symlink_metadata(name)
        .map_err(|error| ProductAssemblyError::io("ASSEMBLY_SOURCE_READ", logical_path, error))?;
    reject_symlink_or_non_file(&metadata, logical_path)?;
    let mut file = open_file_nofollow(&parent, name)
        .map_err(|error| ProductAssemblyError::io("ASSEMBLY_SOURCE_READ", logical_path, error))?;
    let opened = file
        .metadata()
        .map_err(|error| ProductAssemblyError::io("ASSEMBLY_SOURCE_READ", logical_path, error))?;
    reject_symlink_or_non_file(&opened, logical_path)?;
    let bytes = read_bounded(&mut file, logical_path, "ASSEMBLY_SOURCE_READ")?;
    Ok(ProductFile {
        relative_path: relative_path.clone(),
        bytes,
    })
}

pub(crate) fn read_product_tree(
    root: &ProductRoot,
    relative_root: &ProductPath,
    logical_path: &str,
) -> Result<Vec<ProductFile>, ProductAssemblyError> {
    let components = checked_components(relative_root, logical_path)?;
    let directory = open_relative_directory(&root.directory, &components, logical_path)?;
    let mut files = Vec::new();
    let mut total = 0usize;
    walk_directory(
        &directory,
        relative_root,
        logical_path,
        0,
        &mut files,
        &mut total,
    )?;
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

pub(crate) fn total_bytes<'a>(
    files: impl IntoIterator<Item = &'a ProductFile>,
) -> Result<(), ProductAssemblyError> {
    let mut total = 0usize;
    let mut count = 0usize;
    for file in files {
        count = count.saturating_add(1);
        if count > MAX_ASSEMBLY_FILES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_FILE_COUNT_BOUNDS",
                "$",
                format!("one assembly closure is limited to {MAX_ASSEMBLY_FILES} files"),
            ));
        }
        total = total.checked_add(file.bytes.len()).ok_or_else(|| {
            ProductAssemblyError::new(
                "ASSEMBLY_TOTAL_BYTES_BOUNDS",
                "$",
                "assembly byte accounting overflowed",
            )
        })?;
        if total > MAX_ASSEMBLY_TOTAL_BYTES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_TOTAL_BYTES_BOUNDS",
                "$",
                format!("one assembly closure is limited to {MAX_ASSEMBLY_TOTAL_BYTES} bytes"),
            ));
        }
    }
    Ok(())
}

fn walk_directory(
    directory: &Dir,
    relative_root: &ProductPath,
    logical_path: &str,
    depth: usize,
    files: &mut Vec<ProductFile>,
    total: &mut usize,
) -> Result<(), ProductAssemblyError> {
    if depth > MAX_ASSEMBLY_PATH_DEPTH {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_PATH_DEPTH_BOUNDS",
            logical_path,
            format!("assembly paths are limited to {MAX_ASSEMBLY_PATH_DEPTH} components"),
        ));
    }
    let mut entries = directory
        .entries()
        .map_err(|error| ProductAssemblyError::io("ASSEMBLY_DIRECTORY_READ", logical_path, error))?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|error| {
            ProductAssemblyError::io("ASSEMBLY_DIRECTORY_READ", logical_path, error)
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let name = entry.file_name().into_string().map_err(|_| {
            ProductAssemblyError::new(
                "ASSEMBLY_NON_UTF8_PATH",
                logical_path,
                "assembly paths must be UTF-8 for deterministic cross-host receipts",
            )
        })?;
        let relative = ProductPath::parse(format!("{}/{}", relative_root.as_str(), name)).map_err(
            |error| {
                ProductAssemblyError::new("ASSEMBLY_INVALID_PATH", logical_path, error.to_string())
            },
        )?;
        let child_logical = relative.to_string();
        let file_type = entry.file_type().map_err(|error| {
            ProductAssemblyError::io("ASSEMBLY_DIRECTORY_READ", &child_logical, error)
        })?;
        if file_type.is_symlink() {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_SYMLINK_REJECTED",
                &child_logical,
                "assembly input directories may not contain symlinks",
            ));
        }
        if file_type.is_dir() {
            // Open the directory entry itself with no-follow semantics. This
            // keeps the recursive walk on the already-open product capability
            // even if a parent pathname is replaced concurrently.
            let child = open_entry_dir_nofollow(&entry).map_err(|error| {
                ProductAssemblyError::io("ASSEMBLY_DIRECTORY_READ", &child_logical, error)
            })?;
            walk_directory(
                &child,
                &relative,
                &child_logical,
                depth.saturating_add(1),
                files,
                total,
            )?;
            continue;
        }
        if !file_type.is_file() {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_INPUT_SPECIAL_FILE",
                &child_logical,
                "assembly inputs must not contain sockets, devices, or other special files",
            ));
        }
        let mut file = open_entry_nofollow(&entry).map_err(|error| {
            ProductAssemblyError::io("ASSEMBLY_CONTENT_READ", &child_logical, error)
        })?;
        let opened = file.metadata().map_err(|error| {
            ProductAssemblyError::io("ASSEMBLY_CONTENT_READ", &child_logical, error)
        })?;
        reject_symlink_or_non_file(&opened, &child_logical)?;
        let file_bytes = read_bounded(&mut file, &child_logical, "ASSEMBLY_CONTENT_READ")?;
        *total = (*total).checked_add(file_bytes.len()).ok_or_else(|| {
            ProductAssemblyError::new(
                "ASSEMBLY_TOTAL_BYTES_BOUNDS",
                logical_path,
                "assembly byte accounting overflowed",
            )
        })?;
        if *total > MAX_ASSEMBLY_TOTAL_BYTES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_TOTAL_BYTES_BOUNDS",
                logical_path,
                format!("one assembly closure is limited to {MAX_ASSEMBLY_TOTAL_BYTES} bytes"),
            ));
        }
        files.push(ProductFile {
            relative_path: relative,
            bytes: file_bytes,
        });
        if files.len() > MAX_ASSEMBLY_FILES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_FILE_COUNT_BOUNDS",
                logical_path,
                format!("one assembly closure is limited to {MAX_ASSEMBLY_FILES} files"),
            ));
        }
    }
    Ok(())
}

fn checked_components<'a>(
    path: &'a ProductPath,
    logical_path: &str,
) -> Result<Vec<&'a str>, ProductAssemblyError> {
    let components = path.as_str().split('/').collect::<Vec<_>>();
    if components.len() > MAX_ASSEMBLY_PATH_DEPTH {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_PATH_DEPTH_BOUNDS",
            logical_path,
            format!("assembly paths are limited to {MAX_ASSEMBLY_PATH_DEPTH} components"),
        ));
    }
    Ok(components)
}

fn open_relative_directory(
    root: &Dir,
    components: &[&str],
    logical_path: &str,
) -> Result<Dir, ProductAssemblyError> {
    let mut current = root
        .try_clone()
        .map_err(|error| ProductAssemblyError::io("ASSEMBLY_INPUT_READ", logical_path, error))?;
    for component in components {
        current = open_dir_nofollow(&current, component).map_err(|error| {
            ProductAssemblyError::io("ASSEMBLY_INPUT_PARENT", logical_path, error)
        })?;
    }
    Ok(current)
}

/// Opens one directory component relative to an already-open capability
/// without following the named component if it is replaced by a symlink.
/// `cap_std::fs::Dir::open_dir` intentionally follows by default, so all
/// Product Assembly path traversal goes through this helper instead.
pub(crate) fn open_dir_nofollow(parent: &Dir, name: impl AsRef<Path>) -> io::Result<Dir> {
    let parent = parent.try_clone()?.into_std_file();
    let child = cap_primitives::fs::open_dir_nofollow(&parent, name.as_ref())?;
    Ok(Dir::from_std_file(child))
}

fn open_entry_dir_nofollow(entry: &DirEntry) -> io::Result<Dir> {
    let mut options = OpenOptions::new();
    options.read(true);
    options._cap_fs_ext_follow(FollowSymlinks::No);
    let file = entry.open_with(&options)?;
    let std_file = file.into_std();
    if !std_file.metadata()?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "directory entry is not a directory",
        ));
    }
    Ok(Dir::from_std_file(std_file))
}

fn open_file_nofollow(directory: &Dir, name: &str) -> io::Result<cap_std::fs::File> {
    let mut options = OpenOptions::new();
    options.read(true);
    options._cap_fs_ext_follow(FollowSymlinks::No);
    directory.open_with(name, &options)
}

fn open_entry_nofollow(entry: &DirEntry) -> io::Result<cap_std::fs::File> {
    let mut options = OpenOptions::new();
    options.read(true);
    options._cap_fs_ext_follow(FollowSymlinks::No);
    entry.open_with(&options)
}

fn reject_symlink_or_non_file(
    metadata: &cap_std::fs::Metadata,
    logical_path: &str,
) -> Result<(), ProductAssemblyError> {
    if metadata.file_type().is_symlink() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_SYMLINK_REJECTED",
            logical_path,
            "assembly inputs must be regular files and may not be symlinks",
        ));
    }
    if !metadata.is_file() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_INPUT_NOT_FILE",
            logical_path,
            "assembly input must be a regular file",
        ));
    }
    Ok(())
}

fn read_bounded(
    file: &mut cap_std::fs::File,
    logical_path: &str,
    code: &'static str,
) -> Result<Vec<u8>, ProductAssemblyError> {
    let maximum = u64::try_from(MAX_ASSEMBLY_FILE_BYTES)
        .expect("assembly byte bound fits in u64")
        .saturating_add(1);
    let mut bytes = Vec::new();
    file.take(maximum)
        .read_to_end(&mut bytes)
        .map_err(|error| ProductAssemblyError::io(code, logical_path, error))?;
    bounded_bytes(bytes, logical_path)
}

fn bounded_bytes(bytes: Vec<u8>, logical_path: &str) -> Result<Vec<u8>, ProductAssemblyError> {
    if bytes.len() > MAX_ASSEMBLY_FILE_BYTES {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_FILE_BYTES_BOUNDS",
            logical_path,
            format!("one assembly file is limited to {MAX_ASSEMBLY_FILE_BYTES} bytes"),
        ));
    }
    Ok(bytes)
}
