//! `rusty build --release`: a directory a player unpacks and runs, with no
//! `rusty`, SDK, .NET install or checkout. It holds the packed Product, the
//! runtime pack's host with the libraries its window needs and the licences,
//! the .NET runtime of the SDK that built it, the product's tools, and a
//! launcher named after the product.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

/// The release's own top-level directories, replaced on every assembly.
/// Anything else (containers a player installed beside `product.rpak`) stays.
const OWNED: [&str; 5] = ["bin", "lib", "share", "dotnet", "tools"];

/// What the SDK's release targets reported for the product.
pub struct Inputs<'a> {
    pub staged: &'a Path,
    pub launcher_name: &'a str,
    pub tools: &'a Path,
    /// The building SDK's .NET root (`NetCoreRoot`) and its bundled runtime
    /// version (`BundledNETCoreAppPackageVersion`).
    pub dotnet_root: &'a Path,
    pub runtime_version: &'a str,
    /// The runtime pack (a desktop pack for window output) the host comes from.
    pub pack: &'a Path,
}

pub fn assemble(inputs: &Inputs, release: &Path, compress: bool) -> Result<PathBuf, String> {
    let launcher_name = inputs.launcher_name.trim();
    if launcher_name.is_empty()
        || launcher_name.contains(['/', '\\'])
        || matches!(launcher_name, "." | "..")
    {
        return Err(format!(
            "RUSTY_BUILD_RELEASE: RustyEngineProductLauncherName `{launcher_name}` must be a plain file name"
        ));
    }
    let manifest = fs::read(inputs.staged.join("product.json")).map_err(|error| {
        format!("RUSTY_BUILD_RELEASE: could not read the staged product.json: {error}")
    })?;
    let manifest: Value = serde_json::from_slice(&manifest).map_err(|error| {
        format!("RUSTY_BUILD_RELEASE: the staged product.json is invalid JSON: {error}")
    })?;
    let report = product_container::pack_product(inputs.staged, release, compress)
        .map_err(|error| format!("RUSTY_BUILD_RELEASE: {error}"))?;
    println!(
        "Packed {} files ({} bytes) into {}; {} native files beside it",
        report.packed_files,
        report.container_bytes,
        report.container.display(),
        report.loose_files.len()
    );
    // Packing first refuses a release directory that holds the staged
    // Product (or lies inside it) before anything there is removed.
    for owned in OWNED {
        let path = release.join(owned);
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|error| io(&path, error))?;
        }
    }

    // The host, and the libraries it loads from beside itself on Windows
    // (nethost, ANGLE) and with DLSS (NGX's library).
    let pack_bin = inputs.pack.join("bin");
    let bin = release.join("bin");
    fs::create_dir_all(&bin).map_err(|error| io(&bin, error))?;
    let host = format!("rusty-product-host{}", env::consts::EXE_SUFFIX);
    copy_file(&pack_bin.join(&host), &bin.join(&host))?;
    for entry in fs::read_dir(&pack_bin).map_err(|error| io(&pack_bin, error))? {
        let entry = entry.map_err(|error| io(&pack_bin, error))?;
        let name = entry.file_name();
        let library = name
            .to_str()
            .is_some_and(|name| name.ends_with(".dll") || name.contains(".so"));
        if library && entry.path().is_file() {
            copy_file(&entry.path(), &bin.join(&name))?;
        }
    }
    let window = manifest["renderer"]["output"] == "window";
    if window && manifest["renderer"]["windowUi"] == "chromium" {
        copy_tree(&inputs.pack.join("lib/cef"), &release.join("lib/cef"))?;
    }
    // The host serves the UI page from its browser shell in both outputs.
    copy_tree(
        &inputs.pack.join("share/browser"),
        &release.join("share/browser"),
    )?;
    copy_tree(
        &inputs.pack.join("share/third-party"),
        &release.join("share/third-party"),
    )?;

    let tools = tool_names(inputs.tools)?;
    if manifest["coreclr"].is_object() || !tools.is_empty() {
        copy_dotnet(inputs.dotnet_root, inputs.runtime_version, release)?;
    }
    if !tools.is_empty() {
        copy_tree(inputs.tools, &release.join("tools"))?;
    }

    let launcher = release.join(format!("{launcher_name}{}", env::consts::EXE_SUFFIX));
    copy_file(
        &pack_bin.join(format!("rusty-launcher{}", env::consts::EXE_SUFFIX)),
        &launcher,
    )?;
    println!(
        "Release written to {}: run {}{}",
        release.display(),
        launcher.display(),
        if tools.is_empty() {
            String::new()
        } else {
            format!("; tools: {}", tools.join(", "))
        }
    );
    Ok(launcher)
}

/// The tool directories the SDK published, one per tool project.
fn tool_names(tools: &Path) -> Result<Vec<String>, String> {
    if !tools.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(tools).map_err(|error| io(tools, error))? {
        let entry = entry.map_err(|error| io(tools, error))?;
        if entry.path().is_dir() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// The .NET runtime as a private .NET root: its hostfxr, the shared
/// framework the SDK bundles (which a product built by that SDK runs on), and
/// their licences. The host and the tools' apphosts use it from `dotnet/`.
fn copy_dotnet(root: &Path, version: &str, release: &Path) -> Result<(), String> {
    let framework = root.join("shared/Microsoft.NETCore.App").join(version);
    if !framework.is_dir() {
        return Err(format!(
            "RUSTY_BUILD_RELEASE: the SDK's .NET runtime {version} is not at `{}`",
            framework.display()
        ));
    }
    let fxr_versions = root.join("host/fxr");
    let fxr = if fxr_versions.join(version).is_dir() {
        fxr_versions.join(version)
    } else {
        let mut versions: Vec<PathBuf> = fs::read_dir(&fxr_versions)
            .map_err(|error| io(&fxr_versions, error))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_dir())
            .collect();
        versions.sort_by_key(|path| version_key(path));
        versions.pop().ok_or_else(|| {
            format!(
                "RUSTY_BUILD_RELEASE: no hostfxr in `{}`",
                fxr_versions.display()
            )
        })?
    };
    let target = release.join("dotnet");
    copy_tree(
        &fxr,
        &target
            .join("host/fxr")
            .join(fxr.file_name().unwrap_or_default()),
    )?;
    copy_tree(
        &framework,
        &target.join("shared/Microsoft.NETCore.App").join(version),
    )?;
    let licences = release.join("share/third-party/dotnet");
    fs::create_dir_all(&licences).map_err(|error| io(&licences, error))?;
    for file in ["LICENSE.txt", "ThirdPartyNotices.txt"] {
        copy_file(&root.join(file), &licences.join(file))?;
    }
    Ok(())
}

/// `10.0.12` orders as (10, 0, 12); a prerelease suffix is ignored.
fn version_key(path: &Path) -> Vec<u64> {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .split(['.', '-'])
        .map_while(|part| part.parse().ok())
        .collect()
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| io(to, error))?;
    for entry in fs::read_dir(from).map_err(|error| io(from, error))? {
        let entry = entry.map_err(|error| io(from, error))?;
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        if source.is_dir() {
            copy_tree(&source, &target)?;
        } else {
            copy_file(&source, &target)?;
        }
    }
    Ok(())
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to).map(drop).map_err(|error| {
        format!(
            "RUSTY_BUILD_RELEASE: could not copy `{}` to `{}`: {error}",
            from.display(),
            to.display()
        )
    })
}

fn io(path: &Path, error: std::io::Error) -> String {
    format!("RUSTY_BUILD_RELEASE: `{}`: {error}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn a_release_holds_the_host_runtime_tools_and_a_named_launcher() {
        let root = tempfile::tempdir().unwrap();
        let (staged, pack, dotnet, tools, release) = (
            root.path().join("staged"),
            root.path().join("pack"),
            root.path().join("dotnet"),
            root.path().join("tools"),
            root.path().join("release"),
        );
        write(
            &staged.join("product.json"),
            br#"{"product":{"id":"game"},"coreclr":{"assembly":"coreclr/Game.dll"},"ui":{"root":"ui"},"content":{"root":"content"},"renderer":{"output":"window","windowUi":"servo"}}"#,
        );
        write(&staged.join("ui/main.js"), b"ui");
        write(&staged.join("content/a.txt"), b"a");
        write(&staged.join("coreclr/Game.dll"), b"dll");
        let exe = env::consts::EXE_SUFFIX;
        for binary in [
            "rusty-product-host",
            "rusty-launcher",
            "rusty",
            "rusty-scene-render",
        ] {
            write(&pack.join(format!("bin/{binary}{exe}")), binary.as_bytes());
        }
        write(&pack.join("bin/nethost.dll"), b"nethost");
        write(&pack.join("lib/cef/libcef.so"), b"cef");
        write(&pack.join("share/browser/index.html"), b"shell");
        write(&pack.join("share/third-party/NOTICES.txt"), b"notices");
        write(&dotnet.join("host/fxr/10.0.9/libhostfxr.so"), b"old fxr");
        write(&dotnet.join("host/fxr/10.0.12/libhostfxr.so"), b"fxr");
        write(
            &dotnet.join("shared/Microsoft.NETCore.App/10.0.12/System.Runtime.dll"),
            b"runtime",
        );
        write(&dotnet.join("LICENSE.txt"), b"MIT");
        write(&dotnet.join("ThirdPartyNotices.txt"), b"notices");
        write(&tools.join("Import/Import.dll"), b"tool");
        // A container a player installed, and a stale host directory.
        write(&release.join("data.rpak"), b"installed");
        write(&release.join("bin/stale"), b"stale");

        let inputs = Inputs {
            staged: &staged,
            launcher_name: "My Game",
            tools: &tools,
            dotnet_root: &dotnet,
            runtime_version: "10.0.12",
            pack: &pack,
        };
        let launcher = assemble(&inputs, &release, false).unwrap();

        assert_eq!(launcher, release.join(format!("My Game{exe}")));
        assert_eq!(fs::read(&launcher).unwrap(), b"rusty-launcher");
        assert!(release.join("product.rpak").is_file());
        assert!(release.join("coreclr/Game.dll").is_file());
        assert!(release
            .join(format!("bin/rusty-product-host{exe}"))
            .is_file());
        assert!(release.join("bin/nethost.dll").is_file());
        assert!(!release.join(format!("bin/rusty{exe}")).exists());
        assert!(!release
            .join(format!("bin/rusty-scene-render{exe}"))
            .exists());
        assert!(!release.join("bin/stale").exists());
        assert!(!release.join("lib").exists(), "Servo needs no lib/cef");
        assert!(release.join("share/browser/index.html").is_file());
        assert!(release.join("share/third-party/NOTICES.txt").is_file());
        assert!(release
            .join("share/third-party/dotnet/LICENSE.txt")
            .is_file());
        assert!(release
            .join("dotnet/host/fxr/10.0.12/libhostfxr.so")
            .is_file());
        assert!(!release.join("dotnet/host/fxr/10.0.9").exists());
        assert!(release
            .join("dotnet/shared/Microsoft.NETCore.App/10.0.12/System.Runtime.dll")
            .is_file());
        assert!(release.join("tools/Import/Import.dll").is_file());
        assert_eq!(fs::read(release.join("data.rpak")).unwrap(), b"installed");
    }

    #[test]
    fn a_release_holding_the_staged_product_is_refused_before_anything_is_removed() {
        let project = tempfile::tempdir().unwrap();
        let staged = project.path().join("obj/Rusty.Engine/Product");
        write(
            &staged.join("product.json"),
            br#"{"ui":{"root":"ui"},"content":{"root":"content"}}"#,
        );
        write(&staged.join("ui/main.js"), b"ui");
        write(&project.path().join("tools/Import.cs"), b"tool source");
        let inputs = Inputs {
            staged: &staged,
            launcher_name: "Game",
            tools: project.path(),
            dotnet_root: project.path(),
            runtime_version: "10.0.0",
            pack: project.path(),
        };
        let error = assemble(&inputs, project.path(), false).unwrap_err();
        assert!(error.contains("PRODUCT_PACK_OVERLAP"), "{error}");
        assert_eq!(
            fs::read(project.path().join("tools/Import.cs")).unwrap(),
            b"tool source"
        );
    }

    #[test]
    fn a_launcher_name_with_a_separator_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let inputs = Inputs {
            staged: root.path(),
            launcher_name: "../Game",
            tools: root.path(),
            dotnet_root: root.path(),
            runtime_version: "10.0.0",
            pack: root.path(),
        };
        let error = assemble(&inputs, &root.path().join("release"), false).unwrap_err();
        assert!(error.contains("plain file name"), "{error}");
    }
}
