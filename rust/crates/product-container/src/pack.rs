use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{join, write, Body, Bundle, Error, NewEntry, ProductSource};

/// The container's file name in a release directory.
pub const CONTAINER_NAME: &str = "product.rpak";
const MANIFEST: &str = "product.json";
/// The SDK's loose bundle inventory. A container carries the same facts in its
/// own inventory, so this file is not stored.
const BUNDLE_INDEX: &str = ".rusty-bundles.json";

#[derive(Deserialize)]
struct Manifest {
    ui: Root,
    content: Root,
}

#[derive(Deserialize)]
struct Root {
    root: String,
}

#[derive(Deserialize)]
struct BundleIndex {
    bundles: Vec<IndexedBundle>,
}

#[derive(Deserialize)]
struct IndexedBundle {
    id: String,
    root: String,
}

#[derive(Debug)]
pub struct PackReport {
    pub container: PathBuf,
    pub container_bytes: u64,
    pub packed_files: usize,
    /// Files copied loose beside the container (native artifacts).
    pub loose_files: Vec<String>,
}

/// Packs a staged loose Product into `<release>/product.rpak`: the manifest,
/// the UI root and the content root, with bundle membership from the staged
/// bundle inventory. Every other staged file (CoreCLR assemblies, the
/// NativeAOT module) is copied loose beside it, replacing the release's
/// previous copy of each top-level directory. With `compress`, files that zstd
/// shrinks enough are stored compressed (see [`write`]).
pub fn pack_product(staged: &Path, release: &Path, compress: bool) -> Result<PackReport, Error> {
    let source = ProductSource::open(staged)?;
    let ProductSource::Directory(staged) = &source else {
        return Err(Error::NotRegular(staged.display().to_string()));
    };
    // Packing removes and rewrites parts of the release directory, so it must
    // not be the staged Product, lie inside it, or contain it: that would
    // delete the inputs or pack the previous release into the next.
    let resolved = resolve(release)?;
    if resolved.starts_with(staged) || staged.starts_with(&resolved) {
        return Err(Error::Overlap {
            staged: staged.clone(),
            release: resolved,
        });
    }
    let manifest: Manifest =
        serde_json::from_slice(&source.read(MANIFEST)?).map_err(|error| Error::Corrupt {
            path: staged.join(MANIFEST),
            detail: error.to_string(),
        })?;
    let (ui, content) = (manifest.ui.root, manifest.content.root);
    let index = join(&content, BUNDLE_INDEX);
    let bundles = if source.is_file(&index) {
        let index: BundleIndex =
            serde_json::from_slice(&source.read(&index)?).map_err(|error| Error::Corrupt {
                path: staged.join(&index),
                detail: error.to_string(),
            })?;
        index
            .bundles
            .into_iter()
            .map(|bundle| Bundle {
                root: join(&content, &bundle.root),
                id: bundle.id,
            })
            .collect()
    } else {
        Vec::new()
    };

    let mut packed = BTreeSet::from([MANIFEST.to_owned()]);
    packed.extend(source.files(&ui)?);
    packed.extend(source.files(&content)?);
    packed.remove(&index);
    let entries = packed
        .iter()
        .map(|path| NewEntry {
            path: path.clone(),
            bundle: bundles
                .iter()
                .find(|bundle| path.starts_with(&format!("{}/", bundle.root)))
                .map(|bundle| bundle.id.clone()),
            body: Body::File(staged.join(path)),
        })
        .collect();
    let loose: Vec<String> = source
        .files("")?
        .into_iter()
        .filter(|path| !packed.contains(path) && *path != index)
        .collect();

    fs::create_dir_all(release).map_err(Error::io(release))?;
    let container = release.join(CONTAINER_NAME);
    let report = write(&container, entries, bundles, compress)?;
    let tops: BTreeSet<&str> = loose
        .iter()
        .map(|path| path.split('/').next().unwrap_or(path))
        .collect();
    for top in tops {
        let target = release.join(top);
        if target.is_dir() {
            fs::remove_dir_all(&target).map_err(Error::io(&target))?;
        }
    }
    for path in &loose {
        let target = release.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(Error::io(parent))?;
        }
        fs::copy(staged.join(path), &target).map_err(Error::io(&target))?;
    }
    Ok(PackReport {
        container,
        container_bytes: report.bytes,
        packed_files: report.entries,
        loose_files: loose,
    })
}

/// `path` made absolute with symlinks and `..` resolved, also when its
/// trailing directories do not exist yet.
fn resolve(path: &Path) -> Result<PathBuf, Error> {
    let absolute = std::path::absolute(path).map_err(Error::io(path))?;
    let mut existing = absolute.as_path();
    let mut missing = Vec::new();
    while !existing.exists() {
        let name = existing
            .file_name()
            .ok_or_else(|| Error::InvalidPath(path.display().to_string()))?;
        missing.push(name);
        existing = existing.parent().expect("a named path has a parent");
    }
    let mut resolved = fs::canonicalize(existing).map_err(Error::io(existing))?;
    resolved.extend(missing.iter().rev());
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staged_fixture(staged: &Path) {
        for dir in ["ui", "content/rules", "coreclr"] {
            fs::create_dir_all(staged.join(dir)).unwrap();
        }
        fs::write(
            staged.join(MANIFEST),
            br#"{"ui":{"root":"ui"},"content":{"root":"content"}}"#,
        )
        .unwrap();
        fs::write(staged.join("ui/main.js"), b"export {}").unwrap();
        fs::write(staged.join("content/loose.txt"), b"loose").unwrap();
        fs::write(staged.join("content/rules/a.json"), b"{}").unwrap();
        fs::write(
            staged.join("content").join(BUNDLE_INDEX),
            br#"{"bundles":[{"id":"rules","root":"rules","files":[]}]}"#,
        )
        .unwrap();
        fs::write(staged.join("coreclr/Game.dll"), b"MZ").unwrap();
    }

    #[test]
    fn packs_manifest_ui_and_content_and_leaves_native_code_loose() {
        let directory = tempfile::tempdir().unwrap();
        let staged = directory.path().join("staged");
        staged_fixture(&staged);

        let release = directory.path().join("release");
        fs::create_dir_all(release.join("coreclr")).unwrap();
        fs::write(release.join("coreclr/Stale.dll"), b"old").unwrap();
        let report = pack_product(&staged, &release, false).unwrap();
        assert_eq!(report.packed_files, 4);
        assert_eq!(report.loose_files, ["coreclr/Game.dll"]);
        assert!(!release.join("coreclr/Stale.dll").exists());
        assert_eq!(fs::read(release.join("coreclr/Game.dll")).unwrap(), b"MZ");

        let source = ProductSource::open(&release.join(CONTAINER_NAME)).unwrap();
        let container = source.container().unwrap();
        assert_eq!(
            source.files("").unwrap(),
            [
                "content/loose.txt",
                "content/rules/a.json",
                "product.json",
                "ui/main.js"
            ]
        );
        assert_eq!(
            container.bundles(),
            [Bundle {
                id: "rules".into(),
                root: "content/rules".into()
            }]
        );
        assert_eq!(
            container
                .entry("content/rules/a.json")
                .unwrap()
                .bundle
                .as_deref(),
            Some("rules")
        );
        assert_eq!(container.entry("content/loose.txt").unwrap().bundle, None);
    }

    #[test]
    fn a_release_directory_overlapping_the_staged_product_is_refused_untouched() {
        let directory = tempfile::tempdir().unwrap();
        let staged = directory.path().join("staged");
        staged_fixture(&staged);
        let link = directory.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&staged, &link).unwrap();
        let before = ProductSource::open(&staged).unwrap().files("").unwrap();
        let mut overlapping = vec![
            staged.clone(),
            staged.join("release"),
            staged.join("new/../release"),
            directory.path().to_owned(),
        ];
        if cfg!(unix) {
            overlapping.push(link.join("release"));
        }
        for release in overlapping {
            let error = pack_product(&staged, &release, false).unwrap_err();
            assert!(
                matches!(error, Error::Overlap { .. } | Error::InvalidPath(_)),
                "{release:?}: {error}"
            );
            assert_eq!(
                ProductSource::open(&staged).unwrap().files("").unwrap(),
                before
            );
            assert!(!staged.join("release").exists() && !staged.join("product.rpak").exists());
        }
        // A sibling directory still packs, and packs again the same way.
        let release = directory.path().join("release");
        for _ in 0..2 {
            let report = pack_product(&staged, &release, false).unwrap();
            assert_eq!(report.loose_files, ["coreclr/Game.dll"]);
        }
    }
}
