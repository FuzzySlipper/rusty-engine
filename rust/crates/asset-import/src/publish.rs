use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::fingerprint::fingerprint_hex;
use crate::{ImportMode, ImportPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationReceipt {
    pub output_directory: PathBuf,
    pub written_files: Vec<String>,
    pub replaced_previous: bool,
    /// A successful swap can still leave the recoverable old directory behind
    /// if host cleanup fails. Publication is not falsely reported as failed.
    pub retained_backup: Option<PathBuf>,
    /// The previous sidecar can remain as a harmless recoverable backup if
    /// cleanup fails after both authoritative publications succeed.
    pub retained_sidecar_backup: Option<PathBuf>,
}

#[derive(Debug)]
pub enum PublicationError {
    DryRun,
    InvalidPlan,
    UnsafeOutputDirectory,
    ExistingOutputIsNotDirectory(PathBuf),
    SidecarTargetIsNotFile(PathBuf),
    OverlappingTargets {
        output_directory: PathBuf,
        sidecar_path: PathBuf,
    },
    UnsafeRelativePath(String),
    DuplicatePath(String),
    ParentMissing(PathBuf),
    Io(std::io::Error),
    VerificationFailed(String),
    RollbackFailed {
        publish_error: std::io::Error,
        rollback_error: std::io::Error,
        backup: PathBuf,
    },
    SidecarRollbackFailed {
        publication_error: String,
        rollback_error: std::io::Error,
        backup: PathBuf,
    },
}

impl std::fmt::Display for PublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "asset publication failed: {self:?}")
    }
}

impl std::error::Error for PublicationError {}

impl From<std::io::Error> for PublicationError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Publishes all planned output as one directory swap. No output path is
/// touched until every staged file has been written and hash-verified. If the
/// final rename fails, the previous directory is restored before returning.
pub fn publish_directory_atomically(
    plan: &ImportPlan,
    output_directory: &Path,
) -> Result<PublicationReceipt, PublicationError> {
    if plan.mode != ImportMode::Write {
        return Err(PublicationError::DryRun);
    }
    if plan.has_errors || plan.manifest.is_none() || plan.files.is_empty() {
        return Err(PublicationError::InvalidPlan);
    }
    let parent = output_directory
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or(PublicationError::UnsafeOutputDirectory)?;
    let name = output_directory
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .ok_or(PublicationError::UnsafeOutputDirectory)?;
    if !parent.is_dir() {
        return Err(PublicationError::ParentMissing(parent.to_owned()));
    }
    if output_directory.exists() && !output_directory.is_dir() {
        return Err(PublicationError::ExistingOutputIsNotDirectory(
            output_directory.to_owned(),
        ));
    }
    let mut paths = BTreeSet::new();
    for file in &plan.files {
        if !crate::artifact::is_safe_relative_path(&file.relative_path) {
            return Err(PublicationError::UnsafeRelativePath(
                file.relative_path.clone(),
            ));
        }
        if !paths.insert(file.relative_path.as_str()) {
            return Err(PublicationError::DuplicatePath(file.relative_path.clone()));
        }
    }
    validate_file_closure(plan, &paths)?;

    let (staging, backup) = reserve_paths(parent, name)?;
    if let Err(error) = stage_and_verify(plan, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let replaced_previous = output_directory.exists();
    if replaced_previous {
        if let Err(error) = fs::rename(output_directory, &backup) {
            let _ = fs::remove_dir_all(&staging);
            return Err(PublicationError::Io(error));
        }
    }
    if let Err(publish_error) = fs::rename(&staging, output_directory) {
        if replaced_previous {
            if let Err(rollback_error) = fs::rename(&backup, output_directory) {
                return Err(PublicationError::RollbackFailed {
                    publish_error,
                    rollback_error,
                    backup,
                });
            }
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(PublicationError::Io(publish_error));
    }
    let retained_backup = if replaced_previous && fs::remove_dir_all(&backup).is_err() {
        Some(backup)
    } else {
        None
    };
    Ok(PublicationReceipt {
        output_directory: output_directory.to_owned(),
        written_files: paths.into_iter().map(str::to_owned).collect(),
        replaced_previous,
        retained_backup,
        retained_sidecar_backup: None,
    })
}

/// Publishes a provenance sidecar and its generated output as one recoverable
/// transaction. The sidecar is staged before output changes begin. If output
/// publication then fails, the previous sidecar is restored before returning.
pub fn publish_directory_with_sidecar_atomically(
    plan: &ImportPlan,
    output_directory: &Path,
    sidecar_path: &Path,
    sidecar_bytes: &[u8],
) -> Result<PublicationReceipt, PublicationError> {
    if sidecar_path.exists() && !sidecar_path.is_file() {
        return Err(PublicationError::SidecarTargetIsNotFile(
            sidecar_path.to_owned(),
        ));
    }
    let parent = sidecar_path
        .parent()
        .filter(|parent| parent.is_dir())
        .ok_or_else(|| PublicationError::ParentMissing(sidecar_path.to_owned()))?;
    let name = sidecar_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .ok_or(PublicationError::UnsafeOutputDirectory)?;
    reject_overlapping_targets(output_directory, sidecar_path)?;
    let (staging, backup) = reserve_file_paths(parent, name)?;
    if let Err(error) = fs::write(&staging, sidecar_bytes) {
        let _ = fs::remove_file(&staging);
        return Err(PublicationError::Io(error));
    }

    let replaced_sidecar = sidecar_path.exists();
    if replaced_sidecar {
        if let Err(error) = fs::rename(sidecar_path, &backup) {
            let _ = fs::remove_file(&staging);
            return Err(PublicationError::Io(error));
        }
    }
    if let Err(publish_error) = fs::rename(&staging, sidecar_path) {
        if replaced_sidecar {
            if let Err(rollback_error) = fs::rename(&backup, sidecar_path) {
                return Err(PublicationError::RollbackFailed {
                    publish_error,
                    rollback_error,
                    backup,
                });
            }
        }
        let _ = fs::remove_file(&staging);
        return Err(PublicationError::Io(publish_error));
    }

    let mut receipt = match publish_directory_atomically(plan, output_directory) {
        Ok(receipt) => receipt,
        Err(error) => {
            if let Err(rollback_error) = restore_sidecar(sidecar_path, &backup, replaced_sidecar) {
                return Err(PublicationError::SidecarRollbackFailed {
                    publication_error: error.to_string(),
                    rollback_error,
                    backup,
                });
            }
            return Err(error);
        }
    };
    receipt.retained_sidecar_backup = if replaced_sidecar && fs::remove_file(&backup).is_err() {
        Some(backup)
    } else {
        None
    };
    Ok(receipt)
}

fn reject_overlapping_targets(
    output_directory: &Path,
    sidecar_path: &Path,
) -> Result<(), PublicationError> {
    let output = canonical_target(output_directory)?;
    let sidecar = canonical_target(sidecar_path)?;
    if output == sidecar || output.starts_with(&sidecar) || sidecar.starts_with(&output) {
        return Err(PublicationError::OverlappingTargets {
            output_directory: output_directory.to_owned(),
            sidecar_path: sidecar_path.to_owned(),
        });
    }
    Ok(())
}

fn canonical_target(path: &Path) -> Result<PathBuf, PublicationError> {
    if path.exists() {
        return Ok(path.canonicalize()?);
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or(PublicationError::UnsafeOutputDirectory)?;
    let name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or(PublicationError::UnsafeOutputDirectory)?;
    if !parent.is_dir() {
        return Err(PublicationError::ParentMissing(parent.to_owned()));
    }
    let parent = parent.canonicalize()?;
    Ok(parent.join(name))
}

fn reserve_file_paths(parent: &Path, name: &str) -> Result<(PathBuf, PathBuf), PublicationError> {
    for attempt in 0..1000_u32 {
        let suffix = format!("{}-{attempt}", std::process::id());
        let staging = parent.join(format!(".{name}.rusty-stage-{suffix}"));
        let backup = parent.join(format!(".{name}.rusty-backup-{suffix}"));
        if !staging.exists() && !backup.exists() {
            return Ok((staging, backup));
        }
    }
    Err(PublicationError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not reserve sidecar staging paths",
    )))
}

fn restore_sidecar(path: &Path, backup: &Path, had_prior: bool) -> std::io::Result<()> {
    fs::remove_file(path)?;
    if had_prior {
        fs::rename(backup, path)?;
    }
    Ok(())
}

fn reserve_paths(parent: &Path, name: &str) -> Result<(PathBuf, PathBuf), PublicationError> {
    for attempt in 0..1000_u32 {
        let suffix = format!("{}-{attempt}", std::process::id());
        let staging = parent.join(format!(".{name}.rusty-stage-{suffix}"));
        let backup = parent.join(format!(".{name}.rusty-backup-{suffix}"));
        if staging.exists() || backup.exists() {
            continue;
        }
        fs::create_dir(&staging)?;
        return Ok((staging, backup));
    }
    Err(PublicationError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not reserve import staging paths",
    )))
}

fn stage_and_verify(plan: &ImportPlan, staging: &Path) -> Result<(), PublicationError> {
    for file in &plan.files {
        let path = staging.join(&file.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &file.bytes)?;
    }
    let manifest = plan
        .manifest
        .as_ref()
        .ok_or(PublicationError::InvalidPlan)?;
    for artifact in &manifest.artifacts {
        let path = staging.join(&artifact.relative_path);
        let bytes = fs::read(&path)?;
        if bytes.len() as u64 != artifact.byte_len
            || fingerprint_hex(&bytes) != artifact.content_hash
        {
            return Err(PublicationError::VerificationFailed(
                artifact.relative_path.clone(),
            ));
        }
    }
    let name = manifest
        .mesh_asset_id
        .strip_prefix("mesh/")
        .ok_or(PublicationError::InvalidPlan)?;
    let manifest_path = format!("{name}.import.json");
    let expected =
        crate::encode_import_manifest(manifest).map_err(|_| PublicationError::InvalidPlan)?;
    if fs::read(staging.join(&manifest_path))? != expected.as_bytes() {
        return Err(PublicationError::VerificationFailed(manifest_path));
    }
    Ok(())
}

fn validate_file_closure(
    plan: &ImportPlan,
    paths: &BTreeSet<&str>,
) -> Result<(), PublicationError> {
    let manifest = plan
        .manifest
        .as_ref()
        .ok_or(PublicationError::InvalidPlan)?;
    let name = manifest
        .mesh_asset_id
        .strip_prefix("mesh/")
        .ok_or(PublicationError::InvalidPlan)?;
    let manifest_path = format!("{name}.import.json");
    let mut expected: BTreeSet<_> = manifest
        .artifacts
        .iter()
        .map(|artifact| artifact.relative_path.as_str())
        .collect();
    expected.insert(&manifest_path);
    if &expected != paths {
        return Err(PublicationError::InvalidPlan);
    }
    Ok(())
}
