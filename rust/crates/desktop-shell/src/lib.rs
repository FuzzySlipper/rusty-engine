//! The desktop shell: a native window owned by the runtime process, the
//! retained scene presented to it by `render-wgpu`, and the product's
//! TypeScript UI composited over it.
//!
//! The UI is the same browser shell page `rusty dev` serves, rendered off
//! screen by Chromium (`render-wgpu`'s `web-overlay` feature) and imported as
//! a texture whenever it repaints. Window input goes to that page, so the
//! page's existing input capture produces the same normalized runtime input
//! as in a browser, with its UI arbitration unchanged. Chromium's off-screen
//! mode has no pointer lock, so the shell provides it: the page's lock
//! requests grab the native cursor, and raw mouse motion reaches the page as
//! `pointermove` events carrying `movementX`/`movementY`.
//!
//! The shell owns the window, the surface and the draw loop on the main
//! thread. [`DesktopShell::open`] makes the event loop and the device first,
//! so the runtime can build its `Renderer` on [`DesktopShell::gpu`] and start
//! applying publications before the window exists; [`DesktopShell::run`]
//! then opens the window and draws the [`DesktopScene`] each frame.
//!
//! Without the `web-overlay` feature the shell presents the scene with no UI
//! and builds without CEF.

#![forbid(unsafe_code)]

#[cfg(feature = "web-overlay")]
mod keys;
#[cfg(feature = "web-overlay")]
mod overlay;
mod placement;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

pub use render_wgpu::{Gpu, Renderer, RendererOptions};
use render_wgpu::{PresentSkip, WindowSurface};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[cfg(feature = "web-overlay")]
pub use render_wgpu::web::WebRuntimeConfig;

/// The frame interval when the monitor states no refresh rate.
const DEFAULT_REFRESH: Duration = Duration::from_micros(16_667);
/// How long before the display should free the next swapchain image the
/// shell wakes to acquire it. Until then it waits for window events, not in
/// the acquire, so input reaches the page as it arrives.
const ACQUIRE_MARGIN: Duration = Duration::from_millis(2);
/// How often Chromium is pumped between frames. Without its schedule
/// callback (welding does not expose it) the page's own work, such as the
/// request carrying its input to the runtime, waits for the next pump.
#[cfg(feature = "web-overlay")]
const UI_PUMP_INTERVAL: Duration = Duration::from_millis(2);

/// The retained scene the window presents: a renderer built on
/// [`DesktopShell::gpu`].
pub trait DesktopScene: Send + Sync {
    /// Run `draw` with the renderer, the committed scene applied, and the
    /// presentation time to render at (the clock camera samples arrived on).
    /// The shell only encodes and submits inside it: it acquires the
    /// swapchain image before and presents after, so the scene is never held
    /// across a vsync wait. Returns the simulation step the frame showed.
    fn draw(&self, draw: &mut dyn FnMut(&mut Renderer, f64)) -> u64;
    /// The window presented a frame.
    fn presented(&self, frame: PresentedFrame);
    /// A key or mouse button event reached the window at `at`.
    fn input_received(&self, at: SystemTime);
    /// The runtime behind the scene has stopped; the shell closes.
    fn stopped(&self) -> bool;
    /// The window was closed; stop the runtime.
    fn close(&self);
}

/// One presented frame: the step it showed, when it was presented, and how
/// long each stage of the frame took.
#[derive(Debug, Clone, Copy)]
pub struct PresentedFrame {
    pub step: u64,
    pub presented_at: SystemTime,
    /// Waiting for the swapchain image.
    pub acquire: Duration,
    /// Waiting for the scene.
    pub lock: Duration,
    /// Encoding and submitting the frame.
    pub draw: Duration,
    pub present: Duration,
}

#[derive(Debug, Clone)]
pub struct DesktopShellConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    /// The UI page, normally the runtime's browser shell URL. `None` presents
    /// the scene alone.
    pub ui_url: Option<String>,
    /// Where the window's size, position and maximized state are kept between
    /// runs; `None` always opens at `width` × `height`.
    pub placement_file: Option<std::path::PathBuf>,
    #[cfg(feature = "web-overlay")]
    pub web: WebRuntimeConfig,
}

/// Run this process as a Chromium subprocess if Chromium started it as one.
/// Call first in `main`; exit with the returned code when it is `Some`.
#[cfg(feature = "web-overlay")]
pub fn run_web_subprocess(cef_dir: &std::path::Path) -> Result<Option<i32>, String> {
    render_wgpu::web::WebRuntime::run_subprocess(cef_dir)
}

/// What the shell observed, for evidence and diagnostics.
#[derive(Debug, Clone, Default)]
pub struct DesktopShellReport {
    pub adapter: String,
    pub frames_presented: u64,
    pub frames_skipped: u64,
    pub seconds: f64,
    pub ui_imports: u64,
    pub ui_import_errors: u64,
    pub last_ui_error: Option<String>,
}

/// The event loop and device, before the window opens.
pub struct DesktopShell {
    event_loop: EventLoop<()>,
    gpu: Gpu,
}

impl DesktopShell {
    /// Must run on the main thread.
    pub fn open() -> Result<Self, String> {
        let event_loop = EventLoop::new().map_err(|error| error.to_string())?;
        let gpu = Gpu::for_display(event_loop.owned_display_handle())
            .map_err(|error| error.to_string())?;
        Ok(Self { event_loop, gpu })
    }

    /// The device the scene's renderer must be built on.
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// Open the window and present `scene` until the window closes or the
    /// scene stops.
    pub fn run(
        self,
        config: DesktopShellConfig,
        scene: Arc<dyn DesktopScene>,
    ) -> Result<DesktopShellReport, String> {
        self.event_loop.set_control_flow(ControlFlow::Poll);
        let mut shell = Shell {
            config,
            scene,
            gpu: self.gpu,
            window: None,
            started: Instant::now(),
            report: DesktopShellReport::default(),
            error: None,
        };
        self.event_loop
            .run_app(&mut shell)
            .map_err(|error| error.to_string())?;
        if let Some(error) = shell.error {
            return Err(error);
        }
        shell.report.seconds = shell.started.elapsed().as_secs_f64();
        Ok(shell.report)
    }
}

struct Shell {
    config: DesktopShellConfig,
    scene: Arc<dyn DesktopScene>,
    gpu: Gpu,
    window: Option<Open>,
    started: Instant,
    report: DesktopShellReport,
    error: Option<String>,
}

struct Open {
    window: Arc<Window>,
    surface: WindowSurface,
    settle: placement::Settle,
    /// The monitor's frame interval.
    refresh: Duration,
    /// When to wake for the next frame.
    next_frame: Instant,
    #[cfg(feature = "web-overlay")]
    ui: Option<overlay::UiOverlay>,
    #[cfg(feature = "web-overlay")]
    last_pump: Instant,
}

impl Shell {
    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let mut attributes = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(winit::dpi::PhysicalSize::new(
                self.config.width,
                self.config.height,
            ));
        let mut settle = placement::Settle::default();
        if let Some(placement) = self
            .config
            .placement_file
            .as_deref()
            .and_then(placement::Placement::load)
        {
            (attributes, settle) = placement.apply(attributes, event_loop);
        }
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|error| error.to_string())?,
        );
        settle.check(&window, false);
        let size = window.inner_size();
        let surface = WindowSurface::create(&self.gpu, window.clone(), size.width, size.height)
            .map_err(|error| error.to_string())?;
        self.report.adapter = format!("{:?}", self.gpu.adapter_summary());
        #[cfg(feature = "web-overlay")]
        let ui = match &self.config.ui_url {
            Some(url) => Some(overlay::UiOverlay::open(
                &self.config.web,
                &self.gpu,
                url,
                &window,
            )?),
            None => None,
        };
        let refresh = window
            .current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .map_or(DEFAULT_REFRESH, |millihertz| {
                Duration::from_secs_f64(1000.0 / f64::from(millihertz))
            });
        self.window = Some(Open {
            window,
            surface,
            settle,
            refresh,
            next_frame: Instant::now(),
            #[cfg(feature = "web-overlay")]
            ui,
            #[cfg(feature = "web-overlay")]
            last_pump: Instant::now(),
        });
        Ok(())
    }

    fn frame(&mut self) {
        let Some(open) = &mut self.window else { return };
        #[cfg(feature = "web-overlay")]
        if let Some(ui) = &mut open.ui {
            ui.pump(&open.window);
            ui.flush_motion();
            open.last_pump = Instant::now();
        }
        // The vsync wait happens here, before the scene is taken, so a product
        // call applying its changes never waits for the display.
        let acquiring = Instant::now();
        let frame = match open.surface.acquire(&self.gpu) {
            Ok(frame) => frame,
            Err(PresentSkip::Reconfigured | PresentSkip::Unavailable) => {
                self.report.frames_skipped += 1;
                open.next_frame = Instant::now() + open.refresh;
                return;
            }
        };
        let acquired = Instant::now();
        // Under vsync the display frees the next image about one interval
        // after this one.
        open.next_frame = acquired + open.refresh.saturating_sub(ACQUIRE_MARGIN);
        #[cfg(feature = "web-overlay")]
        if let Some(ui) = &mut open.ui {
            ui.page().import_repaint();
        }
        let surface = &mut open.surface;
        // The window's scale factor is the page's device pixel ratio too.
        let pixel_ratio = open.window.scale_factor() as f32;
        #[cfg(feature = "web-overlay")]
        let mut ui = open.ui.as_mut();
        let locking = Instant::now();
        let mut drawn = (locking, locking);
        let step = self.scene.draw(&mut |renderer, now| {
            drawn.0 = Instant::now();
            renderer.set_pixel_ratio(pixel_ratio);
            #[cfg(feature = "web-overlay")]
            if let Some(ui) = ui.as_deref_mut() {
                renderer.render_view_composition_to_frame_with_overlay(
                    surface,
                    &frame,
                    now,
                    ui.page(),
                );
                drawn.1 = Instant::now();
                return;
            }
            renderer.render_view_composition_to_frame(surface, &frame, now);
            drawn.1 = Instant::now();
        });
        let presenting = Instant::now();
        open.surface.present(&self.gpu, frame);
        let presented = Instant::now();
        self.report.frames_presented += 1;
        self.scene.presented(PresentedFrame {
            step,
            presented_at: SystemTime::now(),
            acquire: acquired - acquiring,
            lock: drawn.0 - locking,
            draw: drawn.1 - drawn.0,
            present: presented - presenting,
        });
        #[cfg(feature = "web-overlay")]
        if let Some(ui) = &open.ui {
            let stats = ui.stats();
            self.report.ui_imports = stats.imports;
            self.report.ui_import_errors = stats.import_errors;
            self.report.last_ui_error = ui.last_error();
        }
    }

    fn shut(&mut self, event_loop: &ActiveEventLoop) {
        self.scene.close();
        // The page and Chromium go before the window and device.
        if let Some(open) = self.window.take() {
            if let Some(path) = &self.config.placement_file {
                if let Err(error) = placement::Placement::of(&open.window).save(path) {
                    eprintln!("desktop-shell: could not keep the window placement: {error}");
                }
            }
            #[cfg(feature = "web-overlay")]
            drop(open.ui);
            drop(open.surface);
            drop(open.window);
        }
        event_loop.exit();
    }
}

impl ApplicationHandler for Shell {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if let Err(error) = self.open(event_loop) {
            self.error = Some(error);
            self.shut(event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            self.shut(event_loop);
            return;
        }
        if matches!(
            event,
            WindowEvent::KeyboardInput { .. } | WindowEvent::MouseInput { .. }
        ) {
            self.scene.input_received(SystemTime::now());
        }
        let Some(open) = &mut self.window else { return };
        if let WindowEvent::Resized(size) = event {
            open.surface.resize(&self.gpu, size.width, size.height);
        }
        if let WindowEvent::Moved(_) = event {
            open.settle.check(&open.window, true);
        }
        #[cfg(feature = "web-overlay")]
        if let Some(ui) = &mut open.ui {
            ui.window_event(&open.window, &event);
            // Chromium's work runs only when pumped: do it now rather than
            // at the next frame, so the page sees the input at once.
            if matches!(
                event,
                WindowEvent::KeyboardInput { .. }
                    | WindowEvent::MouseInput { .. }
                    | WindowEvent::MouseWheel { .. }
            ) {
                ui.pump(&open.window);
                open.last_pump = Instant::now();
            }
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        #[cfg(feature = "web-overlay")]
        if let (Some(open), DeviceEvent::MouseMotion { delta }) = (&mut self.window, &event) {
            if let Some(ui) = &mut open.ui {
                ui.raw_motion(delta.0, delta.1);
            }
        }
        let _ = event;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.scene.stopped() {
            self.shut(event_loop);
            return;
        }
        let Some(open) = &mut self.window else { return };
        if Instant::now() >= open.next_frame {
            self.frame();
        } else {
            #[cfg(feature = "web-overlay")]
            if let Some(ui) = &mut open.ui {
                if open.last_pump.elapsed() >= UI_PUMP_INTERVAL {
                    ui.pump(&open.window);
                    open.last_pump = Instant::now();
                }
            }
        }
        // Wait for window events until the next frame (or Chromium pump) is
        // due, rather than blocking in the swapchain acquire.
        let Some(open) = &self.window else { return };
        #[cfg(feature = "web-overlay")]
        let wake = match &open.ui {
            Some(_) => open.next_frame.min(open.last_pump + UI_PUMP_INTERVAL),
            None => open.next_frame,
        };
        #[cfg(not(feature = "web-overlay"))]
        let wake = open.next_frame;
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }
}
