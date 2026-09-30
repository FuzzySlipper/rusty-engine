//! The desktop mode of the runtime process: a native window presenting the
//! world through `render-wgpu`, with the product UI from this host's own
//! browser shell composited over it by Chromium (`desktop-shell`).

use std::path::PathBuf;

/// Where Chromium's runtime files are: a runtime pack keeps them in `lib/cef`
/// beside `bin`. `--cef-dir` overrides it for the browser process; Linux
/// subprocesses load libcef through the dynamic linker and do not read it.
pub(crate) fn cef_dir() -> Result<PathBuf, String> {
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

use csharp_product_runtime::{Gpu, Renderer, SceneDriver, WindowFrame, WindowTiming};
use desktop_shell::{
    DesktopScene, DesktopShell, DesktopShellConfig, PresentedFrame, WebRuntimeConfig,
};

const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 720;

/// The window, opened before the product loads so the runtime builds its
/// renderer on the window's device.
pub(crate) struct Desktop {
    shell: DesktopShell,
    cef_dir: Option<PathBuf>,
    /// Extra Chromium switches for the UI page, `name[=value]`; for example
    /// `remote-debugging-port=9333` lets a CDP client (the playtest harness,
    /// a debugger) attach to the page.
    switches: Vec<String>,
}

impl Desktop {
    /// Opens the event loop and device in window output. Call on the main
    /// thread, before any other thread starts.
    pub(crate) fn open_if_selected(
        output: csharp_product_runtime::RenderOutput,
        cef_dir: Option<PathBuf>,
        switches: &[String],
    ) -> Result<Option<Self>, String> {
        if output != csharp_product_runtime::RenderOutput::Window {
            return Ok(None);
        }
        Ok(Some(Self {
            shell: DesktopShell::open()?,
            cef_dir,
            switches: switches.to_vec(),
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
        timing: Arc<WindowTiming>,
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
        let scene = Arc::new(WindowScene {
            driver,
            timing,
            stopping,
        });
        let report = self.shell.run(
            DesktopShellConfig {
                title,
                width: WINDOW_WIDTH,
                height: WINDOW_HEIGHT,
                ui_url: Some(format!("{}/", origin.trim_end_matches('/'))),
                placement_file: persistence_root.map(|root| root.join("desktop-window")),
                web: WebRuntimeConfig {
                    cef_dir: match self.cef_dir {
                        Some(directory) => directory,
                        None => cef_dir()?,
                    },
                    cache_dir,
                    switches: chromium_switches(&self.switches),
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
fn chromium_switches(extra: &[String]) -> Vec<(String, Option<String>)> {
    // winit prefers Wayland when WAYLAND_DISPLAY names a display.
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty());
    let platform = if wayland { "wayland" } else { "x11" };
    let mut switches = if cfg!(target_os = "linux") {
        vec![("ozone-platform".to_owned(), Some(platform.to_owned()))]
    } else {
        Vec::new()
    };
    switches.extend(
        extra
            .iter()
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
    timing: Arc<WindowTiming>,
    stopping: Arc<AtomicBool>,
}

impl DesktopScene for WindowScene {
    fn draw(&self, draw: &mut dyn FnMut(&mut Renderer, f64)) -> u64 {
        let ((), shown) = self.driver.draw(|renderer, now| draw(renderer, now));
        shown.step
    }

    fn presented(&self, frame: PresentedFrame) {
        self.timing.presented(WindowFrame {
            step: frame.step,
            presented_at: frame.presented_at,
            acquire: frame.acquire,
            lock: frame.lock,
            draw: frame.draw,
            present: frame.present,
        });
    }

    fn input_received(&self, at: std::time::SystemTime) {
        self.timing.input_received(at);
    }

    fn stopped(&self) -> bool {
        self.stopping.load(Ordering::Relaxed)
    }

    fn close(&self) {
        self.stopping.store(true, Ordering::Relaxed);
    }
}
