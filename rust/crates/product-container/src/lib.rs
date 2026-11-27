//! One file holding a built Product's content, and the one reader the runtime
//! uses for a staged Product, loose or packed.
//!
//! A container is a 32-byte header, each file's bytes at a 64-byte-aligned
//! offset, and a JSON inventory: per file its Product-relative path, byte
//! length, SHA-256, offset and bundle membership. The runtime maps it
//! read-only and borrows raw file bytes from the map. A writer may store a
//! file that zstd shrinks enough compressed instead (its stored length is the
//! entry's `zstdLength`); reading it decompresses it. Integrity is the
//! inventory's SHA-256 values: there is no second checksum and, since a pair
//! reads only its own output, no version field beyond the magic. A
//! container's identity is [`collection_identity`] over those values.
//!
//! Native code (CoreCLR assemblies, the NativeAOT module) stays loose beside
//! the container: hostfxr and the dynamic loader take file paths.

mod bytes;
mod container;
mod pack;
mod source;

pub use bytes::{ContentBytes, WeakContentBytes};
pub use container::{write, Body, Bundle, Container, Entry, NewEntry, WriteReport};
pub use pack::{pack_content, pack_product, unpack_content, PackReport, CONTAINER_NAME};
pub use source::ProductSource;

use sha2::{Digest, Sha256};
use std::{fmt, io, path::PathBuf};

/// Why a container or a Product file could not be read or written.
#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
        error: io::Error,
    },
    /// The file does not start with the container magic.
    NotAContainer(PathBuf),
    /// The file is shorter than its header (or the header) records.
    Truncated {
        path: PathBuf,
        expected: u64,
        actual: u64,
    },
    /// The header or inventory is inconsistent with the file.
    Corrupt {
        path: PathBuf,
        detail: String,
    },
    /// The Product has no file at this path.
    Missing(String),
    /// A loose Product entry is a symlink or not a regular file or directory.
    NotRegular(String),
    /// A path is not relative, `/`-separated and normalized.
    InvalidPath(String),
    /// The output is the directory being packed, inside it, or contains it.
    Overlap {
        staged: PathBuf,
        release: PathBuf,
    },
    /// An unpack's output directory already exists.
    Exists(PathBuf),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "PRODUCT_SOURCE_IO",
            Self::NotAContainer(_) => "PRODUCT_CONTAINER_NOT_A_CONTAINER",
            Self::Truncated { .. } => "PRODUCT_CONTAINER_TRUNCATED",
            Self::Corrupt { .. } => "PRODUCT_CONTAINER_CORRUPT",
            Self::Missing(_) => "PRODUCT_SOURCE_MISSING",
            Self::NotRegular(_) => "PRODUCT_SOURCE_NOT_REGULAR",
            Self::InvalidPath(_) => "PRODUCT_SOURCE_PATH",
            Self::Overlap { .. } => "PRODUCT_PACK_OVERLAP",
            Self::Exists(_) => "PRODUCT_UNPACK_EXISTS",
        }
    }

    fn io(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Self {
        let path = path.into();
        move |error| Self::Io { path, error }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.code())?;
        match self {
            Self::Io { path, error } => write!(f, "`{}`: {error}", path.display()),
            Self::NotAContainer(path) => {
                write!(f, "`{}` is not a Product container", path.display())
            }
            Self::Truncated {
                path,
                expected,
                actual,
            } => write!(
                f,
                "`{}` has {actual} bytes; its header records {expected}",
                path.display()
            ),
            Self::Corrupt { path, detail } => write!(f, "`{}`: {detail}", path.display()),
            Self::Missing(path) => write!(f, "the Product has no file `{path}`"),
            Self::NotRegular(path) => {
                write!(
                    f,
                    "`{path}` must be a regular file or directory, not a symlink"
                )
            }
            Self::Overlap { staged, release } => write!(
                f,
                "output `{}` overlaps `{}`, the directory being packed; choose an output outside it",
                release.display(),
                staged.display()
            ),
            Self::Exists(path) => write!(
                f,
                "`{}` already exists; unpack into a new directory",
                path.display()
            ),
            Self::InvalidPath(path) => write!(
                f,
                "`{path}` must be a relative, `/`-separated, normalized path"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// A relative `/`-separated path with no empty, `.` or `..` component.
pub fn is_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// `prefix/path`, or `path` for an empty prefix.
pub fn join(prefix: &str, path: &str) -> String {
    if prefix.is_empty() {
        path.to_owned()
    } else {
        format!("{prefix}/{path}")
    }
}

/// A content collection's identity: SHA-256 over each file's path, a zero
/// byte and its 32-byte SHA-256, in path order. It follows the files and not
/// how they are stored, so a container and the directory it was packed from
/// (or unpacked to) have the same identity.
pub fn collection_identity(mut files: Vec<(&str, [u8; 32])>) -> [u8; 32] {
    files.sort_unstable();
    let mut hasher = Sha256::new();
    for (path, digest) in files {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(digest);
    }
    hasher.finalize().into()
}

/// The 32 bytes of a 64-digit hex SHA-256, as an inventory records it.
pub fn hex_digest(hex: &str) -> Option<[u8; 32]> {
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(digest)
}
