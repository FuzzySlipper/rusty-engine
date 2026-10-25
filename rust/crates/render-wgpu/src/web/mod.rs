//! The desktop shell's UI overlay: the product's web UI, rendered off-screen
//! by a web engine, imported into the renderer's device, and composited over
//! the presented frame in gamma space, as a browser composites its page.
//!
//! Two engines draw the page; the product picks one:
//!
//! - **Chromium** (CEF, `web-overlay`) renders in its own processes and
//!   hands each repaint over as a new GPU texture, imported per repaint.
//!   Off-screen Chromium paints popup widgets (a `<select>`'s list, date
//!   pickers) as a separate surface. The overlay draws it over the page at
//!   its rect while it shows, moved inside the view when it would leave it,
//!   and moves pointer events over it by the same offset. CEF re-executes the
//!   host binary for its subprocesses, so the host calls
//!   [`WebRuntime::run_subprocess`] first in `main`.
//! - **Servo** (`servo-overlay`) runs in this process on the shell's thread
//!   and draws with GL into a pair of colour buffers shared with this
//!   device (`servo_page`): DMA-BUFs imported once each on Linux, D3D11
//!   textures (ANGLE) imported again after each frame on Windows.
//!
//! Either engine runs unsandboxed: it shows the product's own first-party
//! UI, never arbitrary pages.

#[cfg(feature = "servo-overlay")]
mod servo_page;

use std::path::PathBuf;
use std::time::Instant;

#[cfg(feature = "web-overlay")]
use std::path::Path;
#[cfg(feature = "web-overlay")]
use welding::CefSurfaceProducer;
pub use welding::{
    CursorImage, CursorShape, DragOperations, EventModifiers, KeyEvent, KeyEventKind, MouseAction,
    MouseButton, MouseEvent, NavigationEvent,
};
use welding::{HostWgpuContext, ImportedTexture, PopupRect};

#[cfg(feature = "servo-overlay")]
pub use servo_page::SERVO_VERSION;

use crate::{FrameStats, Gpu, Renderer, SurfaceFrame, WindowSurface};

#[cfg(feature = "web-overlay")]
const SANDBOX: welding::CefSandboxMode = welding::CefSandboxMode::UnsandboxedTrustedContent;

/// Which engine draws the page, and how it starts.
#[derive(Debug, Clone)]
pub enum WebEngine {
    /// Off-screen Chromium.
    #[cfg(feature = "web-overlay")]
    Chromium {
        /// The directory holding `libcef`, its `.pak` files, `icudtl.dat` and
        /// `locales/`.
        cef_dir: PathBuf,
        /// Extra Chromium switches, `(name, value)`.
        switches: Vec<(String, Option<String>)>,
    },
    /// Servo, in this process.
    #[cfg(feature = "servo-overlay")]
    Servo {
        /// Servo preferences set by name, `(name, value)`; a value reads as
        /// a bool, integer or float where it parses as one.
        preferences: Vec<(String, String)>,
    },
}

/// How the page's engine is started.
#[derive(Debug, Clone)]
pub struct WebRuntimeConfig {
    pub engine: WebEngine,
    /// The page's profile root: its local storage and cookies persist under
    /// it, as a browser keeps them per origin. Absolute; `None` keeps them in
    /// memory.
    pub cache_dir: Option<PathBuf>,
}

/// The process-wide engine. Dropping it shuts the engine down; drop every
/// [`WebOverlay`] first.
pub struct WebRuntime {
    engine: Engine,
    #[cfg_attr(not(feature = "web-overlay"), allow(dead_code))]
    cache_dir: Option<PathBuf>,
}

enum Engine {
    #[cfg(feature = "web-overlay")]
    Chromium(welding::CefRuntime),
    #[cfg(feature = "servo-overlay")]
    Servo(servo_page::ServoRuntime),
}

impl WebRuntime {
    /// Run this process as a CEF subprocess if CEF started it as one.
    /// Returns the exit code to leave with, or `None` in the host process.
    #[cfg(feature = "web-overlay")]
    pub fn run_subprocess(cef_dir: &Path) -> Result<Option<i32>, String> {
        welding::CefRuntime::execute_process_from(cef_dir, SANDBOX).map_err(|e| e.to_string())
    }

    /// Start the engine. `wake` is called, from any thread, when the engine
    /// has work for [`WebRuntime::pump`]; Chromium is pumped on a timer
    /// instead and never calls it.
    pub fn initialize(
        config: WebRuntimeConfig,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let engine = match config.engine {
            #[cfg(feature = "web-overlay")]
            WebEngine::Chromium { cef_dir, switches } => {
                let _ = wake;
                let mut runtime = welding::CefRuntimeConfig::new(cef_dir, SANDBOX);
                runtime.cache_path = config.cache_dir.clone();
                // The page is the product's own UI: no first-run prompts, no
                // component downloads or background requests of a browser,
                // and no desktop keyring. Keyring access goes over D-Bus to
                // KWallet or libsecret and blocks page loading while that
                // session is locked.
                runtime.command_line_switches = [
                    ("no-first-run", None),
                    ("no-default-browser-check", None),
                    ("disable-component-update", None),
                    ("disable-background-networking", None),
                    ("password-store", Some("basic")),
                ]
                .into_iter()
                .map(|(switch, value)| (switch.to_owned(), value.map(str::to_owned)))
                .collect();
                runtime.command_line_switches.extend(switches);
                Engine::Chromium(
                    welding::CefRuntime::initialize(runtime).map_err(|e| e.to_string())?,
                )
            }
            #[cfg(feature = "servo-overlay")]
            WebEngine::Servo { preferences } => Engine::Servo(servo_page::ServoRuntime::new(
                config
                    .cache_dir
                    .as_ref()
                    .map(|dir| dir.join("servo"))
                    .as_deref(),
                &preferences,
                std::sync::Arc::new(wake),
            )?),
        };
        Ok(Self {
            engine,
            cache_dir: config.cache_dir,
        })
    }

    /// Whether the engine runs only when woken, rather than on a timer.
    pub fn wakes(&self) -> bool {
        match &self.engine {
            #[cfg(feature = "web-overlay")]
            Engine::Chromium(_) => false,
            #[cfg(feature = "servo-overlay")]
            Engine::Servo(_) => true,
        }
    }

    /// Let the engine do its pending work. Chromium needs it about every
    /// 2 ms; Servo when it woke the host.
    pub fn pump(&self) {
        match &self.engine {
            #[cfg(feature = "web-overlay")]
            Engine::Chromium(runtime) => runtime.do_message_loop_work(),
            #[cfg(feature = "servo-overlay")]
            Engine::Servo(runtime) => runtime.pump(),
        }
    }
}

/// What the overlay has imported, for diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WebOverlayStats {
    /// UI repaints imported since the overlay started.
    pub imports: u64,
    /// How long the last import took on the host thread: for Servo, painting
    /// the page and waiting for GL to finish it.
    pub last_import_ms: f64,
    pub import_errors: u64,
}

/// A window key event in the terms either engine needs.
#[derive(Debug, Clone)]
pub struct KeyInput<'a> {
    pub pressed: bool,
    pub repeat: bool,
    /// `KeyboardEvent.code`: the W3C name of the physical key, `KeyW`.
    pub code: &'a str,
    /// `KeyboardEvent.key`: the W3C key value, `w` or `Enter`.
    pub key: &'a str,
    /// The text a press types.
    pub text: Option<&'a str>,
    /// The Windows virtual-key code Chromium uses for `keyCode`.
    pub windows_key_code: i32,
    /// The platform key code Chromium derives `code` from.
    pub native_key_code: i32,
    pub modifiers: EventModifiers,
}

/// A control the page asks the shell to draw over it. Chromium draws its own
/// (as popups); Servo asks for them.
#[derive(Debug, Clone, PartialEq)]
pub enum PageControl {
    /// A `<select>`'s option list, for the element at `rect` (physical
    /// pixels in the view). [`WebOverlay::answer_select`] closes it.
    Select {
        rect: PopupRect,
        options: Vec<PageOption>,
        /// A `<select multiple>`: any number of options can be chosen.
        multiple: bool,
    },
    /// The page closed the control it asked for.
    Closed,
}

/// One option of a `<select>`.
#[derive(Debug, Clone, PartialEq)]
pub struct PageOption {
    /// What [`WebOverlay::answer_select`] chooses it by.
    pub id: usize,
    pub label: String,
    pub disabled: bool,
    pub selected: bool,
    /// Its `<optgroup>`'s label.
    pub group: Option<String>,
}

/// An imported texture and the bind group drawing it at its origin.
struct Layer {
    texture: ImportedTexture,
    bind_group: wgpu::BindGroup,
}

/// The popup widget showing over the page.
struct Popup {
    layer: Layer,
    /// Where Chromium placed it, which can reach past the view.
    requested: PopupRect,
    /// Where it is drawn: `requested` moved inside the view.
    drawn: PopupRect,
}

enum Producer {
    #[cfg(feature = "web-overlay")]
    Chromium(Box<welding::PlatformCefProducer>),
    #[cfg(feature = "servo-overlay")]
    Servo(servo_page::ServoPage),
}

/// One page drawn into a texture of the window's size.
pub struct WebOverlay {
    producer: Producer,
    host: HostWgpuContext,
    /// The page's imported textures: Chromium's latest repaint, or each of
    /// Servo's buffers.
    pages: Vec<Layer>,
    /// The page texture drawn.
    shown: Option<usize>,
    /// The submission that last drew each of `pages`. Servo does not paint
    /// into a buffer until the GPU is done reading it.
    drawn_by: Vec<Option<wgpu::SubmissionIndex>>,
    /// The Servo buffers' generation `pages` holds.
    #[cfg_attr(not(feature = "servo-overlay"), allow(dead_code))]
    imported_generation: u64,
    popup: Option<Popup>,
    /// The view's size in physical pixels.
    size: (u32, u32),
    layout: wgpu::BindGroupLayout,
    shader: wgpu::ShaderModule,
    pipelines: Vec<((wgpu::TextureFormat, bool), wgpu::RenderPipeline)>,
    stats: WebOverlayStats,
    last_error: Option<String>,
}

impl WebOverlay {
    /// Load `url` into a transparent page of `width`×`height` physical pixels
    /// at `scale_factor` physical pixels per CSS pixel. Servo runs
    /// `document_script` at the start of every document; Chromium's page
    /// takes the shell's scripts on its load events instead.
    pub fn new(
        runtime: &WebRuntime,
        gpu: &Gpu,
        url: &str,
        width: u32,
        height: u32,
        scale_factor: f32,
        document_script: Option<&str>,
    ) -> Result<Self, String> {
        let host = HostWgpuContext::new(gpu.device.clone(), gpu.queue.clone());
        let (width, height) = (width.max(1), height.max(1));
        let producer = match &runtime.engine {
            #[cfg(feature = "web-overlay")]
            Engine::Chromium(cef) => Producer::Chromium(Box::new({
                let _ = document_script;
                welding::PlatformCefProducer::new(
                    cef,
                    welding::PlatformCefConfig {
                        surface: welding::CefSurfaceConfig {
                            initial_url: url.to_owned(),
                            initial_size: dpi::PhysicalSize::new(width, height),
                            background_color: None,
                            prefer_accelerated: true,
                            // Chromium creates the profile directory itself.
                            user_data_dir: runtime.cache_dir.as_ref().map(|dir| dir.join("page")),
                            scale_factor,
                            ..Default::default()
                        },
                    },
                    // Windows copies Chromium's D3D11 frames into textures
                    // shared with this device.
                    #[cfg(windows)]
                    &host,
                )
                .map_err(|e| e.to_string())?
            })),
            #[cfg(feature = "servo-overlay")]
            Engine::Servo(servo) => Producer::Servo(servo_page::ServoPage::new(
                servo,
                &host,
                &gpu.adapter.get_info(),
                url,
                dpi::PhysicalSize::new(width, height),
                scale_factor,
                document_script,
            )?),
        };
        let device = &gpu.device;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu web overlay"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render-wgpu web overlay"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        Ok(Self {
            producer,
            host,
            pages: Vec::new(),
            shown: None,
            drawn_by: Vec::new(),
            imported_generation: 0,
            popup: None,
            size: (width, height),
            layout,
            shader,
            pipelines: Vec::new(),
            stats: WebOverlayStats::default(),
            last_error: None,
        })
    }

    /// The engine drawing the page, for readouts: Chromium, or Servo and the
    /// GL renderer it draws with.
    pub fn engine(&self) -> String {
        match &self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(_) => "chromium".to_owned(),
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => format!("{SERVO_VERSION} ({})", page.renderer()),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.size = (width.max(1), height.max(1));
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(producer) => producer
                .resize(dpi::PhysicalSize::new(self.size.0, self.size.1))
                .map_err(|e| e.to_string()),
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => {
                page.resize(self.size.0, self.size.1);
                Ok(())
            }
        }
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) -> Result<(), String> {
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(producer) => producer
                .set_scale_factor(scale_factor)
                .map_err(|e| e.to_string()),
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => {
                page.set_scale_factor(scale_factor);
                Ok(())
            }
        }
    }

    /// Send a pointer event at its position in the view. Over a popup that
    /// was moved into the view, the event moves to where Chromium placed it.
    pub fn send_mouse(&mut self, mut event: MouseEvent) -> Result<(), String> {
        if let Some(popup) = &self.popup {
            if contains(popup.drawn, event.x, event.y) {
                event.x += popup.requested.x - popup.drawn.x;
                event.y += popup.requested.y - popup.drawn.y;
            }
        }
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(producer) => {
                producer.send_mouse_input(event).map_err(|e| e.to_string())
            }
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => {
                page.send_mouse(event);
                Ok(())
            }
        }
    }

    /// Refuse a native drag the page started
    /// ([`NavigationEvent::DragStarted`] at `x`, `y`): Chromium takes no other
    /// input until the embedder ends it. The page gets `dragend` with
    /// `dropEffect` `none`.
    pub fn refuse_drag(&mut self, x: i32, y: i32) -> Result<(), String> {
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(producer) => producer
                .finish_drag_source(x, y, DragOperations::NONE)
                .map_err(|e| e.to_string()),
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(_) => {
                let _ = (x, y);
                Ok(())
            }
        }
    }

    /// Whether a popup widget is showing, and where it is drawn.
    pub fn popup_rect(&self) -> Option<PopupRect> {
        self.popup.as_ref().map(|popup| popup.drawn)
    }

    /// Send a window key event. Chromium gets a raw key event, then a
    /// character event for each character a press types unless Control or
    /// Meta is held; Servo gets one keyboard event.
    pub fn send_key(&mut self, input: &KeyInput<'_>) -> Result<(), String> {
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(producer) => {
                let kind = if input.pressed {
                    KeyEventKind::RawKeyDown
                } else {
                    KeyEventKind::KeyUp
                };
                producer
                    .send_keyboard_input(KeyEvent {
                        kind,
                        windows_key_code: input.windows_key_code,
                        native_key_code: input.native_key_code,
                        character: None,
                        modifiers: input.modifiers,
                    })
                    .map_err(|e| e.to_string())?;
                if input.pressed && !input.modifiers.ctrl && !input.modifiers.meta {
                    for character in input.text.iter().flat_map(|text| text.chars()) {
                        if character.is_control() {
                            continue;
                        }
                        producer
                            .send_keyboard_input(KeyEvent {
                                kind: KeyEventKind::Char,
                                windows_key_code: character as i32,
                                native_key_code: input.native_key_code,
                                character: Some(character),
                                modifiers: input.modifiers,
                            })
                            .map_err(|e| e.to_string())?;
                    }
                }
                Ok(())
            }
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => {
                page.send_key(input);
                Ok(())
            }
        }
    }

    /// Give the page keyboard focus.
    pub fn focus(&mut self) -> Result<(), String> {
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(producer) => producer
                .move_focus(welding::FocusDirection::Forward)
                .map_err(|e| e.to_string()),
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => {
                page.focus();
                Ok(())
            }
        }
    }

    pub fn execute_script(&mut self, script: &str) -> Result<(), String> {
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(producer) => producer
                .execute_script(script, "rusty-desktop-shell")
                .map_err(|e| e.to_string()),
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => {
                page.execute_script(script);
                Ok(())
            }
        }
    }

    /// Navigation, title and console events, oldest first. The shell asks
    /// for no script values or cookies and posts no web messages, so the
    /// page's other callbacks are dropped.
    pub fn poll_event(&mut self) -> Option<NavigationEvent> {
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(producer) => {
                while let Some(event) = producer.poll_web_event() {
                    if let welding::CefSurfaceEvent::Navigation(event) = event {
                        return Some(event);
                    }
                }
                None
            }
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => page.poll_event(),
        }
    }

    /// The next cursor shape the page asks for.
    pub fn poll_cursor(&mut self) -> Option<CursorShape> {
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(producer) => producer.poll_cursor_shape(),
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => page.poll_cursor(),
        }
    }

    /// The next control the page asks the shell to draw. Chromium asks for
    /// none.
    pub fn poll_control(&mut self) -> Option<PageControl> {
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(_) => None,
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => page.poll_control(),
        }
    }

    /// Answer the open [`PageControl::Select`]: choose options `ids`, or
    /// keep the element's choice with `None`.
    pub fn answer_select(&mut self, ids: Option<&[usize]>) {
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(_) => {
                let _ = ids;
            }
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => page.answer_select(ids),
        }
    }

    /// The first gamepad's state in the standard mapping, `None` when no pad
    /// is connected: sticks (down positive) and the 16 buttons' values.
    /// Chromium reads pads itself and ignores it.
    pub fn send_gamepad(&mut self, pad: Option<([f32; 4], [f32; 16])>) {
        match &mut self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(_) => {
                let _ = pad;
            }
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(page) => page.send_gamepad(pad),
        }
    }

    pub fn stats(&self) -> WebOverlayStats {
        self.stats
    }

    /// The last import failure, if the most recent repaint could not be
    /// imported. The previous frame stays on screen.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Bring the page's latest repaint (and Chromium's popup) to the
    /// textures drawn next. It touches no scene state, so do it before
    /// drawing.
    pub fn import_repaint(&mut self) {
        match &self.producer {
            #[cfg(feature = "web-overlay")]
            Producer::Chromium(_) => self.import_chromium(),
            #[cfg(feature = "servo-overlay")]
            Producer::Servo(_) => self.import_servo(),
        }
    }

    /// Import Chromium's page and popup repaints, if they repainted, and drop
    /// a popup Chromium closed.
    #[cfg(feature = "web-overlay")]
    #[allow(irrefutable_let_patterns)]
    fn import_chromium(&mut self) {
        let Producer::Chromium(producer) = &mut self.producer else {
            return;
        };
        let started = Instant::now();
        let frame = producer.acquire_frame(&self.host);
        let popup = producer.acquire_popup(&self.host);
        let requested = producer.popup_rect();
        match frame {
            Ok(Some(frame)) => {
                self.stats.imports += 1;
                self.stats.last_import_ms = started.elapsed().as_secs_f64() * 1e3;
                self.last_error = None;
                self.pages = vec![self.layer(frame, [0, 0], false)];
                self.drawn_by = vec![None];
                self.shown = Some(0);
            }
            Ok(None) => {}
            Err(error) => {
                self.stats.import_errors += 1;
                self.last_error = Some(error.to_string());
            }
        }
        match popup {
            Ok(Some(popup)) => self.popup = Some(self.place_popup(popup.texture, popup.rect)),
            Ok(None) => {}
            Err(error) => {
                self.stats.import_errors += 1;
                self.last_error = Some(error.to_string());
            }
        }
        // Chromium closes a popup without painting: its rect is the signal.
        // A moved popup, or a resized view, keeps the last paint at the new
        // place until the next one arrives.
        match requested {
            None => self.popup = None,
            Some(requested) => {
                if let Some(popup) = self.popup.take() {
                    let placed =
                        popup.requested == requested && popup.drawn == inside(requested, self.size);
                    self.popup = Some(if placed {
                        popup
                    } else {
                        self.place_popup(popup.layer.texture, requested)
                    });
                }
            }
        }
    }

    /// Paint Servo's newest frame, if it has one, into the buffer the GPU no
    /// longer reads, and import the buffers when Servo replaced them.
    #[cfg(feature = "servo-overlay")]
    #[allow(irrefutable_let_patterns)]
    fn import_servo(&mut self) {
        let Producer::Servo(page) = &self.producer else {
            return;
        };
        let painted = page.frame_ready();
        if painted {
            let started = Instant::now();
            let back = page.target().back();
            if let Some(drawn) = self.drawn_by.get_mut(back).and_then(Option::take) {
                let _ = self.host.device.poll(wgpu::PollType::Wait {
                    submission_index: Some(drawn),
                    timeout: None,
                });
            }
            page.paint();
            self.stats.imports += 1;
            self.stats.last_import_ms = started.elapsed().as_secs_f64() * 1e3;
        }
        let target = page.target();
        if target.generation() != self.imported_generation {
            let mut pages = Vec::with_capacity(welding::GL_TARGET_BUFFERS);
            for index in 0..welding::GL_TARGET_BUFFERS {
                let imported = target
                    .export(index)
                    .map_err(|e| e.to_string())
                    .and_then(|frame| {
                        welding::WgpuTextureImporter::import(frame, &self.host)
                            .map_err(|e| e.to_string())
                    });
                match imported {
                    Ok(texture) => pages.push(self.layer(texture, [0, 0], true)),
                    Err(error) => {
                        self.stats.import_errors += 1;
                        self.last_error = Some(error);
                        self.pages.clear();
                        self.shown = None;
                        return;
                    }
                }
            }
            self.imported_generation = target.generation();
            self.pages = pages;
            self.drawn_by = vec![None; welding::GL_TARGET_BUFFERS];
            self.last_error = None;
        } else if cfg!(windows) && painted {
            // A D3D11 texture import acquires it from the external queue, so
            // each finished frame is imported again (about 0.15 ms).
            if let Some(front) = target.front() {
                let imported = target
                    .export(front)
                    .map_err(|e| e.to_string())
                    .and_then(|frame| {
                        welding::WgpuTextureImporter::import(frame, &self.host)
                            .map_err(|e| e.to_string())
                    });
                match imported {
                    Ok(texture) => self.pages[front] = self.layer(texture, [0, 0], true),
                    Err(error) => {
                        self.stats.import_errors += 1;
                        self.last_error = Some(error);
                    }
                }
            }
        }
        self.shown = target.front();
    }

    #[cfg_attr(not(feature = "web-overlay"), allow(dead_code))]
    fn place_popup(&self, texture: ImportedTexture, requested: PopupRect) -> Popup {
        let drawn = inside(requested, self.size);
        Popup {
            layer: self.layer(texture, [drawn.x, drawn.y], false),
            requested,
            drawn,
        }
    }

    /// A bind group drawing `texture` with its top-left at `origin`, its
    /// rows bottom-up when `flipped`.
    fn layer(&self, texture: ImportedTexture, origin: [i32; 2], flipped: bool) -> Layer {
        let device = &self.host.device;
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&origin[0].to_le_bytes());
        bytes[4..8].copy_from_slice(&origin[1].to_le_bytes());
        bytes[8..12].copy_from_slice(&i32::from(flipped).to_le_bytes());
        let uniform = crate::gpu::buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("render-wgpu web overlay origin"),
                contents: &bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu web overlay"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        Layer {
            texture,
            bind_group,
        }
    }

    /// Composite the page, then any popup over it, onto `view`, a non-sRGB
    /// view of the presented image of `target_size`, with premultiplied
    /// alpha.
    fn draw(
        &mut self,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        target_size: (u32, u32),
    ) {
        let page = self.shown.and_then(|shown| self.pages.get(shown));
        let layers: Vec<(bool, Option<PopupRect>)> = [
            page.map(|page| (page.texture.format.is_srgb(), None)),
            self.popup
                .as_ref()
                .map(|popup| (popup.layer.texture.format.is_srgb(), Some(popup.drawn))),
        ]
        .into_iter()
        .flatten()
        .collect();
        if layers.is_empty() {
            return;
        }
        let pipelines: Vec<usize> = layers
            .iter()
            .map(|(srgb, _)| self.pipeline(format, *srgb))
            .collect();
        let page = self.shown.and_then(|shown| self.pages.get(shown));
        let device = &self.host.device;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu web overlay"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu web overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let bind_groups = [
                page.map(|page| &page.bind_group),
                self.popup.as_ref().map(|popup| &popup.layer.bind_group),
            ];
            for ((bind_group, (_, rect)), pipeline) in bind_groups
                .into_iter()
                .flatten()
                .zip(&layers)
                .zip(&pipelines)
            {
                if let Some(rect) = rect {
                    // The popup's rect, clipped to the target.
                    let x0 = rect.x.clamp(0, target_size.0 as i32) as u32;
                    let y0 = rect.y.clamp(0, target_size.1 as i32) as u32;
                    let x1 = (rect.x + rect.width as i32).clamp(0, target_size.0 as i32) as u32;
                    let y1 = (rect.y + rect.height as i32).clamp(0, target_size.1 as i32) as u32;
                    if x1 <= x0 || y1 <= y0 {
                        continue;
                    }
                    pass.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
                }
                pass.set_pipeline(&self.pipelines[*pipeline].1);
                pass.set_bind_group(0, bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        let submitted = self.host.queue.submit([encoder.finish()]);
        if let Some(drawn) = self.shown.and_then(|shown| self.drawn_by.get_mut(shown)) {
            *drawn = Some(submitted);
        }
    }

    /// The pipeline drawing a layer onto `format`, decoding an sRGB-tagged
    /// layer when `srgb`.
    fn pipeline(&mut self, format: wgpu::TextureFormat, srgb: bool) -> usize {
        let key = (format, srgb);
        if let Some(index) = self.pipelines.iter().position(|(k, _)| *k == key) {
            return index;
        }
        let device = &self.host.device;
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu web overlay"),
            bind_group_layouts: &[Some(&self.layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu web overlay"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_overlay"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some(if srgb {
                    "fs_overlay_srgb"
                } else {
                    "fs_overlay"
                }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((key, pipeline));
        self.pipelines.len() - 1
    }
}

/// `rect` moved, without resizing, to lie inside a view of `size` where it
/// fits, else against its top-left.
#[cfg_attr(not(feature = "web-overlay"), allow(dead_code))]
fn inside(rect: PopupRect, size: (u32, u32)) -> PopupRect {
    let fit =
        |position: i32, extent: u32, limit: u32| position.min(limit as i32 - extent as i32).max(0);
    PopupRect {
        x: fit(rect.x, rect.width, size.0),
        y: fit(rect.y, rect.height, size.1),
        ..rect
    }
}

fn contains(rect: PopupRect, x: i32, y: i32) -> bool {
    x >= rect.x && y >= rect.y && x < rect.x + rect.width as i32 && y < rect.y + rect.height as i32
}

impl Renderer {
    /// As [`Renderer::render_view_composition_to_frame`], with `overlay`'s
    /// last imported page composited over the frame
    /// ([`WebOverlay::import_repaint`]). A playing video covers both.
    pub fn render_view_composition_to_frame_with_overlay(
        &mut self,
        surface: &mut WindowSurface,
        frame: &SurfaceFrame,
        time_seconds: f64,
        overlay: &mut WebOverlay,
    ) -> FrameStats {
        let uploaded = self.prepare();
        surface.request(self.samples(), self.vsync());
        self.display_vsync_only = Some(surface.vsync_only());
        self.surface_size = Some(surface.size());
        let (view, finished) = surface.views(frame);
        let mut stats = self.draw_primary(
            view,
            crate::composition::PrimaryDraw::Output,
            |renderer, view| renderer.render_composition(view, time_seconds),
        );
        overlay.draw(finished.color, finished.format, surface.size());
        stats.video = self.draw_video(&finished);
        stats.parts_uploaded = uploaded;
        stats
    }
}

/// A full-window triangle that copies a layer's pixels one to one from its
/// origin (the page at the top-left, a popup at its rect), reading GL's
/// bottom-up rows from the bottom. When the imported texture is sRGB-tagged,
/// loading decodes it; encoding again restores Chromium's premultiplied sRGB
/// bytes for the gamma-space blend.
const SHADER: &str = r#"
@group(0) @binding(0) var page: texture_2d<f32>;
// The layer's top-left in the view, in physical pixels, and in z whether
// its rows run bottom-up.
@group(0) @binding(1) var<uniform> origin: vec4<i32>;

@vertex
fn vs_overlay(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

fn encode(linear: vec3<f32>) -> vec3<f32> {
    let low = linear * 12.92;
    let high = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, linear <= vec3<f32>(0.0031308));
}

fn load(position: vec4<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(page));
    var texel = vec2<i32>(position.xy) - origin.xy;
    if (texel.x < 0 || texel.y < 0 || texel.x >= size.x || texel.y >= size.y) {
        return vec4<f32>(0.0);
    }
    if (origin.z != 0) {
        texel.y = size.y - 1 - texel.y;
    }
    return textureLoad(page, texel, 0);
}

@fragment
fn fs_overlay(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return load(position);
}

@fragment
fn fs_overlay_srgb(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = load(position);
    return vec4<f32>(encode(texel.rgb), texel.a);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: u32, height: u32) -> PopupRect {
        PopupRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn a_popup_past_the_view_moves_inside_it_and_events_follow() {
        let view = (800, 600);
        assert_eq!(inside(rect(40, 70, 220, 300), view), rect(40, 70, 220, 300));
        // Past the bottom-right: moved up and left by the overhang.
        assert_eq!(
            inside(rect(700, 500, 220, 300), view),
            rect(580, 300, 220, 300)
        );
        // Larger than the view: against its top-left.
        assert_eq!(inside(rect(-20, 10, 900, 700), view), rect(0, 0, 900, 700));
        let drawn = rect(580, 300, 220, 300);
        assert!(contains(drawn, 580, 300) && contains(drawn, 799, 599));
        assert!(!contains(drawn, 579, 300) && !contains(drawn, 800, 400));
    }

    /// Servo draws a page with GL into a shared buffer; the device imports
    /// it and the composite shows it the right way up, byte for byte.
    #[cfg(feature = "servo-overlay")]
    #[test]
    fn a_servo_page_reaches_the_device_and_composites_the_right_way_up() {
        use std::time::Duration;

        const WIDTH: u32 = 128;
        const HEIGHT: u32 = 64;
        // A red block at the top-left; the rest of the page is transparent.
        const PAGE: &str = "data:text/html,<body style='margin:0'>\
            <div style='width:64px;height:32px;background:rgb(255,0,0)'></div>";
        let gpu = Gpu::headless().expect("a GPU");
        let runtime = WebRuntime::initialize(
            WebRuntimeConfig {
                engine: WebEngine::Servo {
                    preferences: Vec::new(),
                },
                cache_dir: None,
            },
            || {},
        )
        .expect("Servo starts");
        let mut overlay = WebOverlay::new(&runtime, &gpu, PAGE, WIDTH, HEIGHT, 1.0, None)
            .expect("the page opens");
        let expected = |x: u32, y: u32| {
            if x < 64 && y < 32 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 0, 0]
            }
        };
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut composited = Vec::new();
        while Instant::now() < deadline {
            runtime.pump();
            while overlay.poll_event().is_some() {}
            overlay.import_repaint();
            if overlay.stats().imports == 0 {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            composited = composite(&gpu, &mut overlay, WIDTH, HEIGHT);
            let matches = (0..HEIGHT).all(|y| {
                (0..WIDTH).all(|x| {
                    let at = ((y * WIDTH + x) * 4) as usize;
                    composited[at..at + 4] == expected(x, y)
                })
            });
            if matches {
                assert_eq!(overlay.stats().import_errors, 0);
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let first = composited.get(..4);
        panic!(
            "the page never composited as drawn (first pixel {first:?}, {} repaints)",
            overlay.stats().imports
        );
    }

    /// `overlay` composited over a cleared `width`×`height` target, read back
    /// as RGBA rows, top first.
    #[cfg(feature = "servo-overlay")]
    fn composite(gpu: &Gpu, overlay: &mut WebOverlay, width: u32, height: u32) -> Vec<u8> {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("web overlay test target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        overlay.draw(
            &target.create_view(&Default::default()),
            format,
            (width, height),
        );
        let row = width * 4;
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("web overlay test readback"),
            size: u64::from(row * height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            target.size(),
        );
        gpu.queue.submit([encoder.finish()]);
        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let bytes = slice.get_mapped_range().expect("mapped").to_vec();
        staging.unmap();
        bytes
    }
}
