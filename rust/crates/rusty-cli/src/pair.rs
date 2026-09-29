//! The product's Engine pair pin, the shared pair cache, and installing or
//! updating published pairs.
//!
//! A product pins exactly one pair with `<RustyEnginePackageVersion>` in its
//! `Directory.Build.props`. MSBuild reads that property for the package
//! reference; this module reads and rewrites the same element. Installed pairs
//! are immutable directories in one per-user cache, shared by every product.
//! A directory there exists only after its archive checksum and manifest
//! identity matched, so later builds and launches trust it without network
//! access or another payload pass.

use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;
use sha2::{Digest, Sha256};

pub const PIN_FILE: &str = "Directory.Build.props";
pub const PIN_ELEMENT: &str = "RustyEnginePackageVersion";
pub const CACHE_VARIABLE: &str = "RUSTY_ENGINE_CACHE";
pub const RELEASES_VARIABLE: &str = "RUSTY_ENGINE_RELEASES";
const DEFAULT_RELEASES: &str = "https://github.com/FuzzySlipper/rusty-engine/releases";
const RELEASE_METADATA: &str = "pair-release.json";
const TARGET: &str = "linux-x64";
/// How many `releaseInfo.previous` links an update follows before it gives
/// the source comparison instead.
const MAX_NOTES_CHAIN: usize = 30;

pub fn releases_base() -> String {
    env::var(RELEASES_VARIABLE)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map_or_else(
            || DEFAULT_RELEASES.to_owned(),
            |value| value.trim_end_matches('/').to_owned(),
        )
}

#[derive(Debug, Clone)]
pub struct Pin {
    pub file: PathBuf,
    pub version: String,
}

impl Pin {
    /// The nearest `Directory.Build.props` at or above `start` that declares
    /// the pin element, which is the file MSBuild imports for the product.
    pub fn find(start: &Path) -> Result<Option<Self>, String> {
        for directory in start.ancestors() {
            let file = directory.join(PIN_FILE);
            if !file.is_file() {
                continue;
            }
            let text = fs::read_to_string(&file).map_err(|error| {
                format!("RUSTY_PIN: could not read `{}`: {error}", file.display())
            })?;
            if let Some(version) = read_pin(&text)
                .map_err(|error| format!("RUSTY_PIN: `{}`: {error}", file.display()))?
            {
                return Ok(Some(Self { file, version }));
            }
        }
        Ok(None)
    }

    /// Rewrites only the pin element's value, leaving the rest of the file.
    pub fn write(&self, version: &str) -> Result<(), String> {
        validate_version(version)?;
        let text = fs::read_to_string(&self.file).map_err(|error| {
            format!(
                "RUSTY_PIN: could not read `{}`: {error}",
                self.file.display()
            )
        })?;
        let (start, end) = pin_value_range(&text)
            .map_err(|error| format!("RUSTY_PIN: `{}`: {error}", self.file.display()))?
            .ok_or_else(|| {
                format!(
                    "RUSTY_PIN: `{}` no longer declares <{PIN_ELEMENT}>",
                    self.file.display()
                )
            })?;
        let updated = format!("{}{version}{}", &text[..start], &text[end..]);
        let temporary = self.file.with_extension("props.rusty-update");
        fs::write(&temporary, updated)
            .and_then(|()| fs::rename(&temporary, &self.file))
            .map_err(|error| {
                let _ = fs::remove_file(&temporary);
                format!(
                    "RUSTY_PIN: could not rewrite `{}`: {error}",
                    self.file.display()
                )
            })
    }
}

fn read_pin(text: &str) -> Result<Option<String>, String> {
    let Some((start, end)) = pin_value_range(text)? else {
        return Ok(None);
    };
    let version = text[start..end].trim();
    if version.contains("$(") {
        return Err(format!(
            "<{PIN_ELEMENT}> must be a literal pair version, not `{version}`"
        ));
    }
    validate_version(version)?;
    Ok(Some(version.to_owned()))
}

fn pin_value_range(text: &str) -> Result<Option<(usize, usize)>, String> {
    let open = format!("<{PIN_ELEMENT}>");
    let close = format!("</{PIN_ELEMENT}>");
    let mut occurrences = text.match_indices(&open);
    let Some((open_at, _)) = occurrences.next() else {
        return Ok(None);
    };
    if occurrences.next().is_some() {
        return Err(format!(
            "declares <{PIN_ELEMENT}> more than once; keep one pin"
        ));
    }
    let start = open_at + open.len();
    let end = text[start..]
        .find(&close)
        .map(|offset| start + offset)
        .ok_or_else(|| format!("<{PIN_ELEMENT}> is not closed"))?;
    Ok(Some((start, end)))
}

/// What a product's `Directory.Build.props` declares next to the pin, so every
/// restore (plain `dotnet` included) finds exactly the pinned SDK in the shared
/// cache and an uninstalled pin fails with the fix instead of NU1301.
pub const FEED_DECLARATION: &str = r#"    <RustyEngineCache Condition="'$(RustyEngineCache)' == ''">$(RUSTY_ENGINE_CACHE)</RustyEngineCache>
    <RustyEngineCache Condition="'$(RustyEngineCache)' == '' and '$(XDG_CACHE_HOME)' != ''">$(XDG_CACHE_HOME)/rusty-engine</RustyEngineCache>
    <RustyEngineCache Condition="'$(RustyEngineCache)' == ''">$(HOME)/.cache/rusty-engine</RustyEngineCache>
    <RestoreAdditionalProjectSources>$(RestoreAdditionalProjectSources);$(RustyEngineCache)/pairs/$(RustyEnginePackageVersion)/sdk-feed</RestoreAdditionalProjectSources>
  </PropertyGroup>
  <Target Name="RequireRustyEnginePair" BeforeTargets="Restore;_GenerateRestoreGraph" Condition="!Exists('$(RustyEngineCache)/pairs/$(RustyEnginePackageVersion)/sdk-feed')">
    <Error Text="Rusty Engine pair $(RustyEnginePackageVersion) is not installed: run `rusty install` in this repository." />
  </Target>"#;
const FEED_MARKER: &str = "/pairs/$(RustyEnginePackageVersion)/sdk-feed";
pub const EXACT_VERSION: &str = "[$(RustyEnginePackageVersion)]";
const SHAPE_SKIPPED_DIRECTORIES: &[&str] = &[
    ".git",
    ".runtime",
    "bin",
    "obj",
    "node_modules",
    "target",
    "dist",
    "local",
];

/// Ways the product's project files let a restore pick the wrong SDK: the pair
/// feed is not declared beside the pin, or a `Rusty.Engine` reference is a
/// NuGet minimum rather than exactly the pin.
pub fn shape_problems(pin: &Pin) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    let props = fs::read_to_string(&pin.file).map_err(|error| {
        format!(
            "RUSTY_PIN: could not read `{}`: {error}",
            pin.file.display()
        )
    })?;
    if !props.contains(FEED_MARKER) {
        problems.push(format!(
            "`{}` does not declare the pinned pair's feed, so plain dotnet can restore a different Rusty.Engine. Add after the <{PIN_ELEMENT}> line (closing its PropertyGroup):\n{FEED_DECLARATION}",
            pin.file.display()
        ));
    }
    let root = pin.file.parent().unwrap_or(Path::new("."));
    let mut loose = Vec::new();
    collect_loose_references(root, &mut loose)?;
    for (file, version) in loose {
        problems.push(format!(
            "`{}` references Rusty.Engine as `{version}`, which NuGet treats as a minimum; use Version=\"{EXACT_VERSION}\"",
            file.display()
        ));
    }
    Ok(problems)
}

fn collect_loose_references(
    directory: &Path,
    loose: &mut Vec<(PathBuf, String)>,
) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(());
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !SHAPE_SKIPPED_DIRECTORIES.contains(&name.as_ref()) {
                collect_loose_references(&path, loose)?;
            }
            continue;
        }
        if !(name.ends_with(".csproj") || name.ends_with(".props") || name.ends_with(".targets")) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for version in rusty_engine_reference_versions(&text) {
            if version != EXACT_VERSION {
                loose.push((path.clone(), version));
            }
        }
    }
    Ok(())
}

/// The `Version` of each `<PackageReference Include="Rusty.Engine" ...>`.
fn rusty_engine_reference_versions(text: &str) -> Vec<String> {
    text.match_indices("Include=\"Rusty.Engine\"")
        .filter_map(|(at, _)| {
            let element = &text[at..at + text[at..].find('>')?];
            let start = element.find("Version=\"")? + "Version=\"".len();
            let end = start + element[start..].find('"')?;
            Some(element[start..end].to_owned())
        })
        .collect()
}

pub fn validate_version(version: &str) -> Result<(), String> {
    let valid = !version.is_empty()
        && !version.starts_with('.')
        && version
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'));
    if valid {
        Ok(())
    } else {
        Err(format!(
            "RUSTY_PIN: `{version}` is not a pair version such as 0.1.0-dev.abc123def456"
        ))
    }
}

pub fn cache_root() -> Result<PathBuf, String> {
    if let Some(root) = env::var_os(CACHE_VARIABLE).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(root));
    }
    if let Some(root) = env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(root).join("rusty-engine"));
    }
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".cache/rusty-engine"))
        .ok_or_else(|| format!("RUSTY_CACHE: set HOME or {CACHE_VARIABLE}"))
}

fn pairs_root() -> Result<PathBuf, String> {
    Ok(cache_root()?.join("pairs"))
}

#[derive(Debug, Clone)]
pub struct InstalledPair {
    pub version: String,
    pub root: PathBuf,
}

impl InstalledPair {
    pub fn runtime_pack(&self) -> PathBuf {
        self.root.join("runtime-pack")
    }

    pub fn sdk_feed(&self) -> PathBuf {
        self.root.join("sdk-feed")
    }
}

pub fn installed(version: &str) -> Result<Option<InstalledPair>, String> {
    validate_version(version)?;
    let root = pairs_root()?.join(version);
    Ok(root
        .join("pair-manifest.json")
        .is_file()
        .then(|| InstalledPair {
            version: version.to_owned(),
            root,
        }))
}

pub fn installed_versions() -> Result<Vec<String>, String> {
    let root = pairs_root()?;
    let Ok(entries) = fs::read_dir(&root) else {
        return Ok(Vec::new());
    };
    let mut versions: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("pair-manifest.json").is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    versions.sort();
    Ok(versions)
}

fn archive_name(version: &str) -> String {
    format!("rusty-engine-csharp-pair-{version}-{TARGET}.tar.gz")
}

fn release_asset_url(version: &str, asset: &str) -> String {
    format!("{}/download/csharp-sdk-v{version}/{asset}", releases_base())
}

/// Installs one exact published pair, or reports the one already installed.
pub fn install_version(version: &str) -> Result<(InstalledPair, bool), String> {
    if let Some(pair) = installed(version)? {
        return Ok((pair, false));
    }
    let pairs = pairs_root()?;
    fs::create_dir_all(&pairs).map_err(|error| {
        format!(
            "RUSTY_CACHE: could not create `{}`: {error}",
            pairs.display()
        )
    })?;
    let download = IncomingDirectory::create(&pairs, "download")?;
    let archive = download.path.join(archive_name(version));
    let archive_url = release_asset_url(version, &archive_name(version));
    let checksum = download
        .path
        .join(format!("{}.sha256", archive_name(version)));
    let checksum_url = format!("{archive_url}.sha256");
    eprintln!("rusty: downloading Engine pair {version}");
    if !http_get(&checksum_url, &checksum)? || !http_get(&archive_url, &archive)? {
        return Err(format!(
            "RUSTY_INSTALL_NOT_PUBLISHED: no published pair {version} at {archive_url}. Check the version, or use `rusty update --check` to see the newest pair."
        ));
    }
    let pair = install_archive(&archive)?;
    Ok((pair, true))
}

/// Installs a pair archive that sits next to its `.sha256` file. This is the
/// path for archives obtained another way, and the bootstrap's last step.
pub fn install_archive(archive: &Path) -> Result<InstalledPair, String> {
    let name = archive
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "RUSTY_INSTALL_ARCHIVE: `{}` has no file name",
                archive.display()
            )
        })?;
    let version = name
        .strip_prefix("rusty-engine-csharp-pair-")
        .and_then(|rest| rest.strip_suffix(&format!("-{TARGET}.tar.gz")))
        .ok_or_else(|| {
            format!("RUSTY_INSTALL_ARCHIVE: `{name}` is not named rusty-engine-csharp-pair-<version>-{TARGET}.tar.gz")
        })?;
    validate_version(version)?;
    if let Some(pair) = installed(version)? {
        return Ok(pair);
    }
    verify_checksum(archive)?;

    let pairs = pairs_root()?;
    fs::create_dir_all(&pairs).map_err(|error| {
        format!(
            "RUSTY_CACHE: could not create `{}`: {error}",
            pairs.display()
        )
    })?;
    let incoming = IncomingDirectory::create(&pairs, "extract")?;
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(&incoming.path)
        .status()
        .map_err(|error| format!("RUSTY_PREREQUISITE: could not run tar: {error}"))?;
    if !status.success() {
        return Err(format!(
            "RUSTY_INSTALL_ARCHIVE: tar could not extract `{}` ({status})",
            archive.display()
        ));
    }
    let extracted = incoming.path.join(name.trim_end_matches(".tar.gz"));
    let manifest_path = extracted.join("pair-manifest.json");
    let manifest: Value = fs::read(&manifest_path)
        .map_err(|error| error.to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|error| error.to_string()))
        .map_err(|error| {
            format!("RUSTY_INSTALL_ARCHIVE: `{name}` has no readable pair-manifest.json: {error}")
        })?;
    if manifest["package"]["id"] != "Rusty.Engine" || manifest["package"]["version"] != version {
        return Err(format!(
            "RUSTY_INSTALL_ARCHIVE: `{name}` contains package {} {}, not Rusty.Engine {version}; replace it with the unmodified release archive",
            manifest["package"]["id"], manifest["package"]["version"]
        ));
    }
    let destination = pairs.join(version);
    if let Err(error) = fs::rename(&extracted, &destination) {
        // Another install of the same immutable pair finished first.
        if installed(version)?.is_none() {
            return Err(format!(
                "RUSTY_CACHE: could not move the pair into `{}`: {error}",
                destination.display()
            ));
        }
    }
    installed(version)?.ok_or_else(|| {
        format!(
            "RUSTY_CACHE: `{}` is missing after install",
            destination.display()
        )
    })
}

fn verify_checksum(archive: &Path) -> Result<(), String> {
    let checksum_path = PathBuf::from(format!("{}.sha256", archive.display()));
    let expected = fs::read_to_string(&checksum_path)
        .map_err(|error| {
            format!(
                "RUSTY_INSTALL_CHECKSUM: could not read `{}`: {error}; keep the archive beside its .sha256 file",
                checksum_path.display()
            )
        })?
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut file = fs::File::open(archive).map_err(|error| {
        format!(
            "RUSTY_INSTALL_ARCHIVE: could not open `{}`: {error}",
            archive.display()
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            format!(
                "RUSTY_INSTALL_ARCHIVE: could not read `{}`: {error}",
                archive.display()
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "RUSTY_INSTALL_CHECKSUM: `{}` has SHA-256 {actual}, but its .sha256 names {expected}; download the pair again",
            archive.display()
        ))
    }
}

/// A scratch directory under the pairs root, on the same filesystem as the
/// final location so installation ends with one rename. Removed on drop.
struct IncomingDirectory {
    path: PathBuf,
}

impl IncomingDirectory {
    fn create(pairs: &Path, purpose: &str) -> Result<Self, String> {
        let path = pairs.join(format!(".{purpose}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).map_err(|error| {
            format!(
                "RUSTY_CACHE: could not create `{}`: {error}",
                path.display()
            )
        })?;
        Ok(Self { path })
    }
}

impl Drop for IncomingDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Downloads `url` to `destination`. `Ok(false)` means the server answered
/// 404, which for a release asset means it was never published.
fn http_get(url: &str, destination: &Path) -> Result<bool, String> {
    let output = Command::new("curl")
        .args(["--silent", "--show-error", "--location", "--retry", "3"])
        .args(["--write-out", "%{http_code}", "--output"])
        .arg(destination)
        .arg(url)
        .output()
        .map_err(|error| {
            format!(
                "RUSTY_PREREQUISITE: could not run curl, which install and update need: {error}"
            )
        })?;
    let code = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() {
        return Err(format!(
            "RUSTY_NETWORK: could not fetch {url} (curl {}: {}). Installed pairs keep working offline; `rusty status` lists them.",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    match code.as_str() {
        "200" => Ok(true),
        "404" => {
            let _ = fs::remove_file(destination);
            Ok(false)
        }
        _ => Err(format!("RUSTY_NETWORK: {url} answered HTTP {code}")),
    }
}

/// A published pair's `pair-release.json`, or `None` for a pair published
/// before release metadata existed.
fn fetch_release_metadata(url: &str) -> Result<Option<Value>, String> {
    let scratch = env::temp_dir().join(format!("rusty-release-{}.json", std::process::id()));
    let found = http_get(url, &scratch);
    let bytes = fs::read(&scratch);
    let _ = fs::remove_file(&scratch);
    if !found? {
        return Ok(None);
    }
    let bytes = bytes.map_err(|error| format!("RUSTY_NETWORK: {url}: {error}"))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("RUSTY_RELEASE_METADATA: {url} is not JSON: {error}"))
}

pub fn latest_release() -> Result<Value, String> {
    let url = format!("{}/latest/download/{RELEASE_METADATA}", releases_base());
    fetch_release_metadata(&url)?
        .ok_or_else(|| format!("RUSTY_RELEASE_METADATA: no latest published pair at {url}"))
}

pub fn release(version: &str) -> Result<Option<Value>, String> {
    validate_version(version)?;
    fetch_release_metadata(&release_asset_url(version, RELEASE_METADATA))
}

/// Lines describing what changed from `current` to the pair `target`: each
/// intermediate pair's release notes, newest first, then how to see anything
/// the chain cannot reach.
pub fn release_notes_chain(target: &Value, current: &str) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let mut current_release = target.clone();
    for _ in 0..MAX_NOTES_CHAIN {
        let version = current_release["version"].as_str().unwrap_or("unknown");
        let info = &current_release["releaseInfo"];
        if info.is_null() {
            lines.push(format!(
                "  {version}: published without release information"
            ));
            break;
        }
        lines.push(format!(
            "  {version}: {}",
            info["notes"].as_str().unwrap_or("no notes link")
        ));
        if let Some(diff) = info["apiDiff"].as_str() {
            lines.push(format!("    public API diff: {diff}"));
        }
        let Some(previous) = info["previous"]["version"].as_str() else {
            lines.push("  (no earlier pair is recorded)".to_owned());
            break;
        };
        if previous == current {
            return Ok(lines);
        }
        match release(previous)? {
            Some(next) => current_release = next,
            None => {
                lines.push(format!(
                    "  {previous}: published without release information"
                ));
                break;
            }
        }
    }
    if let Some(compare) = compare_url(current, target) {
        lines.push(format!("  every Engine change since your pin: {compare}"));
    }
    Ok(lines)
}

fn compare_url(current: &str, target: &Value) -> Option<String> {
    let from = current.rsplit('.').next()?;
    if from.len() < 7 || !from.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }
    let to = target["sourceRevision"].as_str()?;
    let repository = releases_base();
    let repository = repository.strip_suffix("/releases")?;
    Some(format!("{repository}/compare/{from}...{to}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROPS: &str = "<Project>\n  <PropertyGroup>\n    <!-- pin -->\n    <RustyEnginePackageVersion>0.1.0-dev.aaaaaaaaaaaa</RustyEnginePackageVersion>\n    <Other>kept</Other>\n  </PropertyGroup>\n</Project>\n";

    #[test]
    fn pin_reads_the_single_literal_version() {
        assert_eq!(
            read_pin(PROPS).unwrap().as_deref(),
            Some("0.1.0-dev.aaaaaaaaaaaa")
        );
        assert_eq!(read_pin("<Project/>").unwrap(), None);
    }

    #[test]
    fn pin_rejects_duplicates_and_msbuild_expressions() {
        let twice = format!("{PROPS}{PROPS}");
        assert!(read_pin(&twice).unwrap_err().contains("more than once"));
        let expression = "<RustyEnginePackageVersion>$(Other)</RustyEnginePackageVersion>";
        assert!(read_pin(expression).unwrap_err().contains("literal"));
    }

    #[test]
    fn pin_write_replaces_only_the_version() {
        let root = env::temp_dir().join(format!("rusty-pin-write-{}", std::process::id()));
        let nested = root.join("src/Game");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join(PIN_FILE), PROPS).unwrap();

        let pin = Pin::find(&nested).unwrap().expect("pin above the project");
        assert_eq!(pin.file, root.join(PIN_FILE));
        pin.write("0.1.0-dev.bbbbbbbbbbbb").unwrap();

        let rewritten = fs::read_to_string(root.join(PIN_FILE)).unwrap();
        assert_eq!(
            rewritten,
            PROPS.replace("0.1.0-dev.aaaaaaaaaaaa", "0.1.0-dev.bbbbbbbbbbbb")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shape_flags_a_missing_feed_and_minimum_references() {
        let root = env::temp_dir().join(format!("rusty-shape-{}", std::process::id()));
        fs::create_dir_all(root.join("src/Game")).unwrap();
        fs::write(root.join(PIN_FILE), PROPS).unwrap();
        fs::write(
            root.join("src/Game/Game.csproj"),
            r#"<PackageReference Include="Rusty.Engine" Version="$(RustyEnginePackageVersion)" />"#,
        )
        .unwrap();
        let pin = Pin::find(&root).unwrap().unwrap();
        let problems = shape_problems(&pin).unwrap();
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("RestoreAdditionalProjectSources"));
        assert!(problems[1].contains("minimum"));

        fs::write(
            root.join(PIN_FILE),
            PROPS.replace(
                "    <Other>kept</Other>\n",
                &format!("{FEED_DECLARATION}\n  <PropertyGroup>\n"),
            ),
        )
        .unwrap();
        fs::write(
            root.join("src/Game/Game.csproj"),
            r#"<PackageReference Include="Rusty.Engine" Version="[$(RustyEnginePackageVersion)]">"#,
        )
        .unwrap();
        assert!(shape_problems(&pin).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn versions_are_single_path_segments() {
        assert!(validate_version("0.1.0-dev.abc123def456").is_ok());
        assert!(validate_version("0.1.0-dev.playtest-20260928c").is_ok());
        for bad in ["", "../x", "a/b", ".hidden", "1 2"] {
            assert!(validate_version(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn compare_link_uses_the_pinned_revision_suffix() {
        let target = serde_json::json!({ "sourceRevision": "f".repeat(40) });
        assert_eq!(
            compare_url("0.1.0-dev.abc123def456", &target).as_deref(),
            Some(&*format!(
                "https://github.com/FuzzySlipper/rusty-engine/compare/abc123def456...{}",
                "f".repeat(40)
            ))
        );
        assert_eq!(compare_url("0.1.0-dev.playtest-20260928c", &target), None);
    }
}
