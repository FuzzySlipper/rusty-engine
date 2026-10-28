//! The desktop mode of the runtime process: a native window presenting the
//! world through `render-wgpu`, with the product UI from this host's own
//! browser shell composited over it by Chromium or Servo (`desktop-shell`).

use std::path::PathBuf;

/// Where Chromium's runtime files are: a runtime pack keeps them in `lib/cef`
/// beside `bin`. The host loads libcef from here only when Chromium draws the
/// UI, so a release whose window UI is Servo leaves the directory out.
/// `--cef-dir` overrides it for the browser process; Chromium's Linux
/// subprocesses take libcef from `lib/cef` or the host's rpath.
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

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use csharp_product_runtime::{Gpu, Renderer, SceneDriver, WindowFrame, WindowTiming};
use desktop_shell::{
    DesktopScene, DesktopShell, DesktopShellConfig, PresentedFrame, UiRepaints, WebEngine,
    WebRuntimeConfig,
};
use product_host::ProductHostWindowInput;

use crate::product_bundle::WindowUi;

/// The window, opened before the product loads so the runtime builds its
/// renderer on the window's device.
pub(crate) struct Desktop {
    shell: DesktopShell,
    /// The web engine drawing the UI page.
    ui: WindowUi,
    /// The product's windowed size with none remembered
    /// (`renderer.windowSize`); `None` follows the monitor.
    windowed_size: Option<(u32, u32)>,
    cef_dir: Option<PathBuf>,
    /// Extra Chromium switches for the UI page, `name[=value]`; for example
    /// `remote-debugging-port=9333` lets a CDP client (the playtest harness,
    /// a debugger) attach to the page.
    switches: Vec<String>,
    /// Servo preferences for the UI page, `name=value`.
    #[cfg_attr(not(feature = "servo"), allow(dead_code))]
    servo_preferences: Vec<String>,
    /// The monitor's refresh, which the window reports once it opens.
    cadence: csharp_product_runtime::PresentationCadence,
}

impl Desktop {
    /// Opens the event loop and device in window output. Call on the main
    /// thread, before any other thread starts.
    pub(crate) fn open_if_selected(
        output: csharp_product_runtime::RenderOutput,
        ui: WindowUi,
        windowed_size: Option<(u32, u32)>,
        cef_dir: Option<PathBuf>,
        switches: &[String],
        servo_preferences: &[String],
    ) -> Result<Option<Self>, String> {
        if output != csharp_product_runtime::RenderOutput::Window {
            return Ok(None);
        }
        if ui == WindowUi::Servo && !cfg!(feature = "servo") {
            return Err(
                "the product's window UI is servo (renderer.windowUi), and this desktop host \
                 was built without Servo"
                    .to_owned(),
            );
        }
        Ok(Some(Self {
            shell: DesktopShell::open()?,
            ui,
            windowed_size,
            cef_dir,
            switches: switches.to_vec(),
            servo_preferences: servo_preferences.to_vec(),
            cadence: csharp_product_runtime::PresentationCadence::default(),
        }))
    }

    pub(crate) fn gpu(&self) -> Gpu {
        self.shell.gpu().clone()
    }

    /// The display cadence the runtime presents tweens at.
    pub(crate) fn presentation_cadence(&self) -> csharp_product_runtime::PresentationCadence {
        self.cadence.clone()
    }

    /// Show the world and the host's browser shell page at `origin` until
    /// the window closes or `stopping` is set. Closing the window sets
    /// `closed`, which ends the host.
    /// The page's storage persists under `persistence_root`, as the
    /// product's own saves do.
    pub(crate) fn run(
        self,
        title: String,
        origin: &str,
        persistence_root: Option<&std::path::Path>,
        runtime: WindowRuntime,
        stopping: Arc<AtomicBool>,
        closed: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let WindowRuntime {
            driver,
            timing,
            input: window_input,
        } = runtime;
        let cache_dir = match persistence_root {
            Some(root) => {
                let dir = std::path::absolute(root.join("desktop-ui"))
                    .map_err(|error| error.to_string())?;
                std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
                Some(dir)
            }
            None => None,
        };
        // The product's request with the player's choices over it.
        let mode = driver.settings_readout().requested.window_mode;
        let scene = Arc::new(WindowScene {
            driver,
            timing,
            window_input,
            drawn_seconds: AtomicU64::new(0),
            cadence: self.cadence,
            stopping,
            closed,
        });
        let report = self.shell.run(
            DesktopShellConfig {
                title,
                mode,
                windowed_size: self.windowed_size,
                ui_url: Some(format!("{}/", origin.trim_end_matches('/'))),
                placement_file: persistence_root.map(|root| root.join("desktop-window")),
                web: WebRuntimeConfig {
                    engine: match self.ui {
                        WindowUi::Chromium => WebEngine::Chromium {
                            cef_dir: match self.cef_dir {
                                Some(directory) => directory,
                                None => cef_dir()?,
                            },
                            switches: chromium_switches(&self.switches),
                        },
                        #[cfg(feature = "servo")]
                        WindowUi::Servo => WebEngine::Servo {
                            preferences: self
                                .servo_preferences
                                .iter()
                                .filter_map(|preference| preference.split_once('='))
                                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                                .collect(),
                        },
                        #[cfg(not(feature = "servo"))]
                        WindowUi::Servo => {
                            return Err("this desktop host was built without Servo".to_owned())
                        }
                    },
                    cache_dir,
                },
            },
            scene,
        )?;
        println!(
            "RUSTY_DESKTOP {{\"ui\":{:?},\"framesPresented\":{},\"framesSkipped\":{},\"seconds\":{:.1},\"uiImports\":{},\"uiImportErrors\":{}}}",
            report.ui_engine.as_deref().unwrap_or(""),
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

/// What the window takes from the runtime: the scene it draws, where it
/// reports its frames, and where it hands gameplay input.
pub(crate) struct WindowRuntime {
    pub(crate) driver: Arc<SceneDriver>,
    pub(crate) timing: Arc<WindowTiming>,
    pub(crate) input: ProductHostWindowInput,
}

struct WindowScene {
    driver: Arc<SceneDriver>,
    timing: Arc<WindowTiming>,
    /// Where the window hands gameplay input while it holds the lock.
    window_input: ProductHostWindowInput,
    /// The presentation time the frame being presented drew at, as f64 bits.
    drawn_seconds: AtomicU64,
    cadence: csharp_product_runtime::PresentationCadence,
    stopping: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
}

impl DesktopScene for WindowScene {
    fn draw(&self, draw: &mut dyn FnMut(&mut Renderer, f64)) -> u64 {
        let ((), shown) = self.driver.draw(|renderer, now| draw(renderer, now));
        self.drawn_seconds
            .store(shown.presentation_seconds.to_bits(), Ordering::Relaxed);
        shown.step
    }

    fn presented(&self, frame: PresentedFrame) {
        self.timing.presented(WindowFrame {
            step: frame.step,
            presentation_seconds: f64::from_bits(self.drawn_seconds.load(Ordering::Relaxed)),
            presented_at: frame.presented_at,
            acquire: frame.acquire,
            lock: frame.lock,
            draw: frame.draw,
            present: frame.present,
        });
    }

    fn ui_opened(&self, engine: &str) {
        self.timing.ui_opened(engine);
    }

    fn ui_repainted(&self, repaints: UiRepaints) {
        self.timing.ui_repainted(
            repaints.imports,
            repaints.import_errors,
            repaints.last_cost,
            repaints.last_shown_at,
        );
    }

    fn page_script(&self) -> Option<(u64, String)> {
        self.timing.page_script()
    }

    fn page_answered(&self, id: u64, answer: Result<String, String>) {
        self.timing.page_answered(id, answer);
    }

    fn input_received(&self, at: std::time::SystemTime) {
        self.timing.input_received(at);
    }

    fn route_gameplay(&self, routed: bool, controller: bool) {
        self.window_input.route(routed, controller);
    }

    fn gameplay(&self, facts: Vec<runtime_input::RuntimeInputFact>) {
        self.window_input.facts(facts);
    }

    fn refresh(&self, interval: std::time::Duration) {
        self.cadence.set(interval);
    }

    fn stopped(&self) -> bool {
        self.stopping.load(Ordering::Relaxed)
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}
