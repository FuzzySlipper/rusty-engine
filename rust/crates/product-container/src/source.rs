use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{is_relative_path, join, Container, Error};

/// A staged Product: a loose directory (`rusty dev`) or a container with its
/// native artifacts loose beside it (a release). Every read of the Product's
/// manifest, UI and content goes through this.
#[derive(Debug, Clone)]
pub enum ProductSource {
    Directory(PathBuf),
    Container {
        container: Arc<Container>,
        beside: PathBuf,
    },
}

impl ProductSource {
    /// A directory is a loose Product; a regular file is a container.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let metadata = fs::symlink_metadata(path).map_err(Error::io(path))?;
        let canonical = || fs::canonicalize(path).map_err(Error::io(path));
        if metadata.is_dir() {
            Ok(Self::Directory(canonical()?))
        } else if metadata.is_file() {
            let container = Container::open(path)?;
            let beside = canonical()?
                .parent()
                .expect("a canonical file path has a parent")
                .to_owned();
            Ok(Self::Container {
                container: Arc::new(container),
                beside,
            })
        } else {
            Err(Error::NotRegular(path.display().to_string()))
        }
    }

    /// Where the manifest's native artifact paths resolve: the Product
    /// directory, or the directory holding the container.
    pub fn native_root(&self) -> &Path {
        match self {
            Self::Directory(root) => root,
            Self::Container { beside, .. } => beside,
        }
    }

    pub fn container(&self) -> Option<&Container> {
        match self {
            Self::Directory(_) => None,
            Self::Container { container, .. } => Some(container),
        }
    }

    /// The file's bytes: read from disk, or borrowed from the container map.
    pub fn read(&self, path: &str) -> Result<Cow<'_, [u8]>, Error> {
        if !is_relative_path(path) {
            return Err(Error::InvalidPath(path.to_owned()));
        }
        match self {
            Self::Directory(root) => {
                let file = inside(root, path)?;
                if !fs::metadata(&file).map_err(Error::io(&file))?.is_file() {
                    return Err(Error::NotRegular(path.to_owned()));
                }
                fs::read(&file).map(Cow::Owned).map_err(Error::io(file))
            }
            Self::Container { container, .. } => container.get(path),
        }
    }

    pub fn is_file(&self, path: &str) -> bool {
        match self {
            Self::Directory(root) => {
                inside(root, path).is_ok_and(|file| file.metadata().is_ok_and(|m| m.is_file()))
            }
            Self::Container { container, .. } => container.entry(path).is_some(),
        }
    }

    /// Whether `path` is a directory. A container stores files only, so there
    /// every valid path that is not a file is a directory, possibly empty.
    pub fn is_dir(&self, path: &str) -> bool {
        match self {
            Self::Directory(root) => {
                inside(root, path).is_ok_and(|dir| dir.metadata().is_ok_and(|m| m.is_dir()))
            }
            Self::Container { container, .. } => {
                is_relative_path(path) && container.entry(path).is_none()
            }
        }
    }

    /// Every file under `prefix` (the whole Product for ""), Product-relative
    /// and sorted. A loose Product may hold only regular files and
    /// directories, with no symlinks.
    pub fn files(&self, prefix: &str) -> Result<Vec<String>, Error> {
        if !prefix.is_empty() && !is_relative_path(prefix) {
            return Err(Error::InvalidPath(prefix.to_owned()));
        }
        let mut files = Vec::new();
        match self {
            Self::Directory(root) => {
                if !prefix.is_empty() {
                    inside(root, prefix)?;
                }
                walk(root, prefix, &mut files)?
            }
            Self::Container { container, .. } => {
                let under = format!("{prefix}/");
                files.extend(
                    container
                        .entries()
                        .iter()
                        .filter(|entry| prefix.is_empty() || entry.path.starts_with(&under))
                        .map(|entry| entry.path.clone()),
                );
            }
        }
        files.sort();
        Ok(files)
    }
}

/// `root/path`, when it is no symlink and resolves inside `root`: a
/// symlinked parent directory must not lead outside the Product.
fn inside(root: &Path, path: &str) -> Result<PathBuf, Error> {
    if !is_relative_path(path) {
        return Err(Error::InvalidPath(path.to_owned()));
    }
    let joined = root.join(path);
    let metadata = fs::symlink_metadata(&joined).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::Missing(path.to_owned())
        } else {
            Error::io(&joined)(error)
        }
    })?;
    let canonical = fs::canonicalize(&joined).map_err(Error::io(&joined))?;
    if metadata.file_type().is_symlink() || !canonical.starts_with(root) {
        return Err(Error::NotRegular(path.to_owned()));
    }
    Ok(joined)
}

fn walk(root: &Path, relative: &str, files: &mut Vec<String>) -> Result<(), Error> {
    let directory = if relative.is_empty() {
        root.to_owned()
    } else {
        root.join(relative)
    };
    for item in fs::read_dir(&directory).map_err(Error::io(&directory))? {
        let item = item.map_err(Error::io(&directory))?;
        let name = item.file_name();
        let name = name
            .to_str()
            .filter(|name| is_relative_path(name))
            .ok_or_else(|| Error::InvalidPath(item.path().display().to_string()))?;
        let path = join(relative, name);
        let file_type = item.file_type().map_err(Error::io(item.path()))?;
        if file_type.is_dir() {
            walk(root, &path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        } else {
            return Err(Error::NotRegular(path));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{write, Body, Bundle, NewEntry};

    fn loose(root: &Path) {
        fs::create_dir_all(root.join("ui/assets")).unwrap();
        fs::create_dir_all(root.join("content/rules")).unwrap();
        fs::write(root.join("product.json"), b"{}").unwrap();
        fs::write(root.join("ui/main.js"), b"export {}").unwrap();
        fs::write(root.join("ui/assets/a.css"), b"a{}").unwrap();
        fs::write(root.join("content/rules/a.json"), b"{\"id\":1}").unwrap();
        fs::write(root.join("content/table.json"), "[1,2,3],".repeat(4096)).unwrap();
    }

    fn packed(root: &Path, out: &Path, compress: bool) {
        let source = ProductSource::open(root).unwrap();
        let entries = source
            .files("")
            .unwrap()
            .into_iter()
            .map(|path| NewEntry {
                bundle: path.starts_with("content/rules/").then(|| "rules".into()),
                body: Body::File(root.join(&path)),
                path,
            })
            .collect();
        let bundles = vec![Bundle {
            id: "rules".into(),
            root: "content/rules".into(),
        }];
        write(out, entries, bundles, compress).unwrap();
    }

    #[test]
    fn loose_and_packed_list_and_read_the_same_files() {
        for compress in [false, true] {
            parity(compress);
        }
    }

    fn parity(compress: bool) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("product");
        loose(&root);
        let out = directory.path().join("release.rpak");
        packed(&root, &out, compress);
        let loose = ProductSource::open(&root).unwrap();
        let packed = ProductSource::open(&out).unwrap();
        let table = packed.container().unwrap().entry("content/table.json");
        assert_eq!(table.unwrap().zstd_length.is_some(), compress);
        assert_eq!(
            packed.native_root(),
            fs::canonicalize(directory.path()).unwrap()
        );
        for prefix in ["", "ui", "content"] {
            assert_eq!(loose.files(prefix).unwrap(), packed.files(prefix).unwrap());
        }
        assert_eq!(
            loose.files("ui").unwrap(),
            ["ui/assets/a.css", "ui/main.js"]
        );
        for path in loose.files("").unwrap() {
            assert_eq!(loose.read(&path).unwrap(), packed.read(&path).unwrap());
            assert!(loose.is_file(&path) && packed.is_file(&path));
        }
        for source in [&loose, &packed] {
            assert!(source.is_dir("ui/assets") && source.is_dir("content"));
            assert!(!source.is_dir("ui/main.js") && !source.is_file("ui"));
            assert!(!source.is_dir("../ui") && !source.is_file("/product.json"));
            assert!(matches!(source.read("ui/none.js"), Err(Error::Missing(_))));
            assert!(matches!(source.read("../x"), Err(Error::InvalidPath(_))));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_loose_product_refuses_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("product");
        loose(&root);
        fs::write(directory.path().join("outside"), b"x").unwrap();
        std::os::unix::fs::symlink(directory.path().join("outside"), root.join("ui/link")).unwrap();
        let source = ProductSource::open(&root).unwrap();
        assert!(matches!(source.files("ui"), Err(Error::NotRegular(_))));
        assert!(matches!(source.read("ui/link"), Err(Error::NotRegular(_))));
        std::os::unix::fs::symlink(directory.path(), root.join("escape")).unwrap();
        assert!(matches!(
            source.read("escape/outside"),
            Err(Error::NotRegular(_))
        ));
        assert!(!source.is_file("escape/outside") && !source.is_dir("escape"));
    }
}
