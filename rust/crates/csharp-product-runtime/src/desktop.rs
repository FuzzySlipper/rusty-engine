//! The desktop mode of the runtime process: a native window presenting the
//! world through `render-wgpu`, with the product UI from this host's own
//! browser shell composited over it by Chromium (`desktop-shell`).

use std::path::PathBuf;

/// Overrides where Chromium's runtime files are; a runtime pack keeps them in
/// `lib/cef` beside `bin`.
const CEF_DIR_ENV: &str = "RUSTY_CEF_DIR";

pub(crate) fn cef_dir() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os(CEF_DIR_ENV) {
        return Ok(PathBuf::from(dir));
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    executable
        .parent()
        .and_then(|bin| bin.parent())
        .map(|root| root.join("lib/cef"))
        .ok_or_else(|| "the host executable has no runtime pack root".to_owned())
}

/// Chromium starts its renderer, GPU and utility processes by re-running
/// this executable; those exit here with Chromium's code.
pub(crate) fn run_chromium_subprocess() -> Result<(), String> {
    if !std::env::args().any(|arg| arg.starts_with("--type=")) {
        return Ok(());
    }
    if let Some(code) = desktop_shell::run_web_subprocess(&cef_dir()?)? {
        std::process::exit(code);
    }
    Ok(())
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use csharp_product_runtime::{Gpu, Renderer, SceneDriver};
use desktop_shell::{DesktopScene, DesktopShell, DesktopShellConfig, WebRuntimeConfig};

const AUDIO_OUTPUT_ENV: &str = "RUSTY_AUDIO_OUTPUT";
/// Extra Chromium switches for the UI page, comma-separated `name[=value]`;
/// for example `remote-debugging-port=9333` lets a CDP client (the playtest
/// harness, a debugger) attach to the page.
const CHROMIUM_SWITCHES_ENV: &str = "RUSTY_CEF_SWITCHES";
const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 720;

/// The window, opened before the product loads so the runtime builds its
/// renderer on the window's device.
pub(crate) struct Desktop {
    shell: DesktopShell,
}

impl Desktop {
    /// Opens the event loop and device when `RUSTY_RENDER_OUTPUT=window`.
    /// Call on the main thread, before any other thread starts.
    pub(crate) fn open_if_selected() -> Result<Option<Self>, String> {
        if csharp_product_runtime::render_output_mode().map_err(|error| error.to_string())?
            != Some("window")
        {
            return Ok(None);
        }
        // A desktop application plays its own sound.
        if std::env::var_os(AUDIO_OUTPUT_ENV).is_none() {
            std::env::set_var(AUDIO_OUTPUT_ENV, "device");
        }
        Ok(Some(Self {
            shell: DesktopShell::open()?,
        }))
    }

    pub(crate) fn gpu(&self) -> Gpu {
        self.shell.gpu().clone()
    }

    /// Show the world and the host's browser shell page at `origin` until
    /// the window closes or `stopping` is set. Closing the window sets
    /// `stopping`, which ends the host.
    /// The page's storage persists under `persistence_root`, as the
    /// product's own saves do.
    pub(crate) fn run(
        self,
        title: String,
        origin: &str,
        persistence_root: Option<&std::path::Path>,
        driver: Arc<SceneDriver>,
        stopping: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let cache_dir = match persistence_root {
            Some(root) => {
                let dir = std::path::absolute(root.join("desktop-ui"))
                    .map_err(|error| error.to_string())?;
                std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
                Some(dir)
            }
            None => None,
        };
        let scene = Arc::new(WindowScene { driver, stopping });
        let report = self.shell.run(
            DesktopShellConfig {
                title,
                width: WINDOW_WIDTH,
                height: WINDOW_HEIGHT,
                ui_url: Some(format!("{}/", origin.trim_end_matches('/'))),
                web: WebRuntimeConfig {
                    cef_dir: cef_dir()?,
                    cache_dir,
                    switches: chromium_switches(),
                },
            },
            scene,
        )?;
        println!(
            "RUSTY_DESKTOP {{\"framesPresented\":{},\"framesSkipped\":{},\"seconds\":{:.1},\"uiImports\":{},\"uiImportErrors\":{}}}",
            report.frames_presented,
            report.frames_skipped,
            report.seconds,
            report.ui_imports,
            report.ui_import_errors
        );
        Ok(())
    }
}

/// Chromium renders off-screen but still opens the display it runs on;
/// point it at the one the window uses.
fn chromium_switches() -> Vec<(String, Option<String>)> {
    // winit prefers Wayland when WAYLAND_DISPLAY names a display.
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty());
    let platform = if wayland { "wayland" } else { "x11" };
    let mut switches = if cfg!(target_os = "linux") {
        vec![("ozone-platform".to_owned(), Some(platform.to_owned()))]
    } else {
        Vec::new()
    };
    let extra = std::env::var(CHROMIUM_SWITCHES_ENV).unwrap_or_default();
    switches.extend(
        extra
            .split(',')
            .filter(|switch| !switch.is_empty())
            .map(|switch| match switch.split_once('=') {
                Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
                None => (switch.to_owned(), None),
            }),
    );
    switches
}

struct WindowScene {
    driver: Arc<SceneDriver>,
    stopping: Arc<AtomicBool>,
}

impl DesktopScene for WindowScene {
    fn draw(&self, draw: &mut dyn FnMut(&mut Renderer, f64)) {
        self.driver.draw(|renderer, now| draw(renderer, now));
    }

    fn stopped(&self) -> bool {
        self.stopping.load(Ordering::Relaxed)
    }

    fn close(&self) {
        self.stopping.store(true, Ordering::Relaxed);
    }
}
