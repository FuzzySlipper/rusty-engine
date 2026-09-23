//! Owns an optional headless Chromium process for a foreground product host.

use std::{
    env, fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static NEXT_PROFILE_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) struct HeadlessBrowser {
    child: Child,
    profile: PathBuf,
    stopped: bool,
}

impl HeadlessBrowser {
    pub(crate) fn launch(url: &str) -> Result<Self, String> {
        let executable = chromium_executable()?;
        let profile = create_profile_directory()?;
        let mut command = Command::new(&executable);
        command
            .arg("--headless=new")
            .arg("--no-sandbox")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-gpu-sandbox")
            .arg("--use-gl=angle")
            .arg("--use-angle=swiftshader")
            .arg("--enable-unsafe-swiftshader")
            .arg("--enable-webgl")
            .arg("--ignore-gpu-blocklist")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-background-networking")
            .arg("--disable-extensions")
            .arg("--disable-sync")
            .arg("--hide-scrollbars")
            .arg("--window-size=1280,720")
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // Give Chromium and its renderer/GPU subprocesses a private group
            // so host shutdown can terminate the whole browser process tree.
            command.process_group(0);
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = fs::remove_dir_all(&profile);
                return Err(format!(
                    "RUSTY_HEADLESS_BROWSER_START: could not start `{}`: {error}",
                    executable.display()
                ));
            }
        };

        // Catch invalid browser flags, unusable profiles, and immediate
        // process startup failures while preserving the useful exit status.
        thread::sleep(Duration::from_millis(150));
        match child.try_wait() {
            Ok(Some(status)) => {
                let _ = stop_browser_processes(&mut child);
                let _ = fs::remove_dir_all(&profile);
                return Err(format!(
                    "RUSTY_HEADLESS_BROWSER_EXIT: `{}` exited during startup with {status}",
                    executable.display()
                ));
            }
            Ok(None) => {}
            Err(error) => {
                let _ = stop_browser_processes(&mut child);
                let _ = fs::remove_dir_all(&profile);
                return Err(format!(
                    "RUSTY_HEADLESS_BROWSER_WAIT: could not inspect `{}` startup: {error}",
                    executable.display()
                ));
            }
        }

        eprintln!(
            "RUSTY_HEADLESS_BROWSER started pid={} executable={} url={} profile={}",
            child.id(),
            executable.display(),
            url,
            profile.display()
        );
        Ok(Self {
            child,
            profile,
            stopped: false,
        })
    }

    pub(crate) fn shutdown(mut self) -> Result<(), String> {
        self.stop().map_err(|error| {
            format!(
                "RUSTY_HEADLESS_BROWSER_CLEANUP: could not stop Chromium or remove profile `{}`: {error}",
                self.profile.display()
            )
        })
    }

    fn stop(&mut self) -> std::io::Result<()> {
        if self.stopped {
            return Ok(());
        }
        self.stopped = true;
        let stop_result = stop_browser_processes(&mut self.child);
        let profile_result = match fs::remove_dir_all(&self.profile) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        };
        stop_result?;
        profile_result
    }
}

fn stop_browser_processes(child: &mut Child) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let process_group = format!("-{}", child.id());
        let termination = signal_process_group("TERM", &process_group);
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut wait_error = None;
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => thread::sleep(Duration::from_millis(25)),
                Err(error) => {
                    wait_error = Some(error);
                    break;
                }
            }
        }

        // Chromium can leave renderer or GPU helpers briefly alive after its
        // browser process exits. Signal the isolated group again before
        // reaping the direct child and deleting its disposable profile.
        let force_termination = signal_process_group("KILL", &process_group);
        if termination.is_err() || force_termination.is_err() {
            let _ = child.kill();
        }
        let child_wait = child.wait().map(|_| ());
        termination?;
        force_termination?;
        if let Some(error) = wait_error {
            return Err(error);
        }
        child_wait
    }

    #[cfg(not(unix))]
    {
        if child.try_wait()?.is_none() {
            let _ = child.kill();
        }
        child.wait().map(|_| ())
    }
}

#[cfg(unix)]
fn signal_process_group(signal: &str, process_group: &str) -> std::io::Result<()> {
    let _ = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg("--")
        .arg(process_group)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(())
}

impl Drop for HeadlessBrowser {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!(
                "RUSTY_HEADLESS_BROWSER_CLEANUP: could not stop Chromium or remove profile `{}`: {error}",
                self.profile.display()
            );
        }
    }
}

fn chromium_executable() -> Result<PathBuf, String> {
    if let Some(configured) = env::var_os("RUSTY_CHROMIUM_PATH") {
        let path = PathBuf::from(configured);
        if !path.is_file() {
            return Err(format!(
                "RUSTY_HEADLESS_BROWSER_PATH: RUSTY_CHROMIUM_PATH does not name a file: `{}`",
                path.display()
            ));
        }
        return Ok(path);
    }

    let path_entries = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_default();
    for name in [
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
    ] {
        for directory in &path_entries {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err("RUSTY_HEADLESS_BROWSER_MISSING: install Chromium or set RUSTY_CHROMIUM_PATH to its executable".to_owned())
}

fn create_profile_directory() -> Result<PathBuf, String> {
    let base = env::temp_dir();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for _ in 0..16 {
        let ordinal = NEXT_PROFILE_ID.fetch_add(1, Ordering::Relaxed);
        let profile = base.join(format!(
            "rusty-product-headless-{}-{timestamp}-{ordinal}",
            std::process::id()
        ));
        match fs::create_dir(&profile) {
            Ok(()) => return Ok(profile),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "RUSTY_HEADLESS_PROFILE: could not create `{}`: {error}",
                    profile.display()
                ));
            }
        }
    }
    Err(
        "RUSTY_HEADLESS_PROFILE: could not allocate a unique temporary profile directory"
            .to_owned(),
    )
}
