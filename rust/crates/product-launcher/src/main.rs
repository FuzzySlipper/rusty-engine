//! The executable at the root of a release (`rusty build --release`), copied
//! there under the product's own name. It starts the release's host on the
//! release's `product.rpak`, with the loader the product declares and the
//! player's data directory for the product as the persistence root. Further
//! arguments pass through to the host. On Windows it opens no console window:
//! the host runs without one and writes its output to `last-run.log` in the
//! persistence root.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use product_container::{ProductSource, CONTAINER_NAME};
use serde_json::Value;

fn main() -> ExitCode {
    match launch() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn launch() -> Result<ExitCode, String> {
    let executable = env::current_exe()
        .map_err(|error| format!("RUSTY_LAUNCHER: could not locate this launcher: {error}"))?;
    let release = executable
        .parent()
        .ok_or("RUSTY_LAUNCHER: the launcher has no directory")?;
    let product = release.join(CONTAINER_NAME);
    let manifest = ProductSource::open(&product)
        .and_then(|source| source.read("product.json").map(|bytes| bytes.into_owned()))
        .map_err(|error| {
            format!(
                "RUSTY_LAUNCHER: could not read `{}`: {error}",
                product.display()
            )
        })?;
    let manifest: Value = serde_json::from_slice(&manifest).map_err(|error| {
        format!(
            "RUSTY_LAUNCHER: `{}` holds an invalid product.json: {error}",
            product.display()
        )
    })?;
    let id = manifest["product"]["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or("RUSTY_LAUNCHER: the product declares no id")?;
    let loader = if manifest["nativeAot"].is_object() {
        "nativeaot"
    } else {
        "coreclr"
    };
    let persistence = data_directory()?.join(id);
    let host = release.join(format!("bin/rusty-product-host{}", env::consts::EXE_SUFFIX));
    let mut command = Command::new(&host);
    command
        .arg("--product")
        .arg(&product)
        .args(["--loader", loader])
        .arg("--persistence-root")
        .arg(&persistence)
        .args(env::args_os().skip(1));
    run(command, &host, &persistence)
}

/// Where the player's data for each product lives: `XDG_DATA_HOME`, else
/// `~/.local/share`, on Linux; `%LOCALAPPDATA%` on Windows.
fn data_directory() -> Result<PathBuf, String> {
    let directory = if cfg!(windows) {
        env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".local/share")))
    };
    directory.filter(|path| path.is_absolute()).ok_or_else(|| {
        "RUSTY_LAUNCHER: no user data directory (HOME or LOCALAPPDATA) is set".to_owned()
    })
}

#[cfg(unix)]
fn run(mut command: Command, host: &Path, _persistence: &Path) -> Result<ExitCode, String> {
    use std::os::unix::process::CommandExt;
    let error = command.exec();
    Err(format!(
        "RUSTY_LAUNCHER: could not start `{}`: {error}",
        host.display()
    ))
}

#[cfg(windows)]
fn run(mut command: Command, host: &Path, persistence: &Path) -> Result<ExitCode, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::fs::create_dir_all(persistence).map_err(|error| {
        format!(
            "RUSTY_LAUNCHER: could not create `{}`: {error}",
            persistence.display()
        )
    })?;
    let log_path = persistence.join("last-run.log");
    let log = std::fs::File::create(&log_path).map_err(|error| {
        format!(
            "RUSTY_LAUNCHER: could not create `{}`: {error}",
            log_path.display()
        )
    })?;
    let error_log = log
        .try_clone()
        .map_err(|error| format!("RUSTY_LAUNCHER: {error}"))?;
    let status = command
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(log)
        .stderr(error_log)
        .status()
        .map_err(|error| {
            format!(
                "RUSTY_LAUNCHER: could not start `{}`: {error}",
                host.display()
            )
        })?;
    Ok(status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .map_or(ExitCode::FAILURE, ExitCode::from))
}
