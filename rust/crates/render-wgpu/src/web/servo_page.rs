//! The UI page in Servo: an embedded web engine on this process's main
//! thread, drawing with GL into colour buffers this crate's device imports
//! without a copy (`welding::GlSharedTarget`).
//!
//! Servo calls the embedder back through its delegate; the delegate queues
//! what the shell polls, in the shapes the Chromium page reports them, so the
//! shell treats both engines alike.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use dpi::PhysicalSize;
use servo::{
    Code, ConsoleLogLevel, Cursor, DeviceIntRect, DevicePoint, EventLoopWaker, InputEvent, Key,
    KeyState, KeyboardEvent, LoadStatus, Location, Modifiers, MouseButtonAction, MouseButtonEvent,
    MouseLeftViewportEvent, MouseMoveEvent, PrefValue, RenderingContext, RgbaImage, Servo,
    ServoBuilder, WebView, WebViewBuilder, WebViewDelegate, WebViewPoint, WheelDelta, WheelEvent,
    WheelMode,
};
use welding::GlSharedTarget;

use super::{CursorShape, KeyInput, MouseAction, MouseButton, MouseEvent, NavigationEvent};

/// The Servo version this crate embeds, for readouts.
pub const SERVO_VERSION: &str = "servo 0.7.0";

/// Wakes the host's event loop when Servo has work for it.
struct Waker(Arc<dyn Fn() + Send + Sync>);

impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Waker(Arc::clone(&self.0)))
    }

    fn wake(&self) {
        (self.0)();
    }
}

/// The process's Servo. Dropping it shuts Servo down; drop every page first.
pub(super) struct ServoRuntime {
    servo: Servo,
}

impl ServoRuntime {
    /// Servo with the page's storage under `cache_dir` (in memory without
    /// one), `preferences` set by name, and `wake` called from any thread
    /// when [`ServoRuntime::pump`] has work.
    pub(super) fn new(
        cache_dir: Option<&Path>,
        preferences: &[(String, String)],
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        // The host links both of rustls's providers (Servo's network stack
        // brings aws-lc-rs, the rest of the host ring), so rustls has no
        // default for Servo to take: ring, which the host already uses. An
        // already installed default stays.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let prefs = servo::Preferences {
            // The page is composited over the scene: what it leaves undrawn
            // stays transparent.
            shell_background_color_rgba: [0.0, 0.0, 0.0, 0.0],
            ..Default::default()
        };
        let opts = servo::Opts {
            config_dir: cache_dir.map(Path::to_path_buf),
            temporary_storage: cache_dir.is_none(),
            ..Default::default()
        };
        let servo = ServoBuilder::default()
            .opts(opts)
            .preferences(prefs)
            .event_loop_waker(Box::new(Waker(wake)))
            .build();
        for (name, value) in preferences {
            servo.set_preference(name, preference_value(value));
        }
        Ok(Self { servo })
    }

    pub(super) fn pump(&self) {
        self.servo.spin_event_loop();
    }
}

/// A preference's value as typed on a command line.
fn preference_value(text: &str) -> PrefValue {
    if let Ok(value) = text.parse::<bool>() {
        PrefValue::Bool(value)
    } else if let Ok(value) = text.parse::<i64>() {
        PrefValue::Int(value)
    } else if let Ok(value) = text.parse::<f64>() {
        PrefValue::Float(value)
    } else {
        PrefValue::Str(text.to_owned())
    }
}

/// What Servo told the page since the shell last asked.
#[derive(Default)]
struct Delegate {
    frame_ready: Cell<bool>,
    events: RefCell<VecDeque<NavigationEvent>>,
    cursors: RefCell<VecDeque<CursorShape>>,
}

impl WebViewDelegate for Delegate {
    fn notify_new_frame_ready(&self, _: WebView) {
        self.frame_ready.set(true);
    }

    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {
        let url = webview.url().map(String::from).unwrap_or_default();
        let event = match status {
            LoadStatus::Started => NavigationEvent::LoadStart { url },
            LoadStatus::Complete => NavigationEvent::LoadEnd {
                url,
                http_status: 200,
            },
            _ => return,
        };
        self.events.borrow_mut().push_back(event);
    }

    fn notify_cursor_changed(&self, _: WebView, cursor: Cursor) {
        self.cursors.borrow_mut().push_back(cursor_shape(cursor));
    }

    fn show_console_message(&self, _: WebView, level: ConsoleLogLevel, message: String) {
        // Chromium's levels: 0 verbose, 1 info, 2 warning, 3 error.
        let level = match level {
            ConsoleLogLevel::Log
            | ConsoleLogLevel::Debug
            | ConsoleLogLevel::Trace
            | ConsoleLogLevel::Dir => 0,
            ConsoleLogLevel::Info => 1,
            ConsoleLogLevel::Warn => 2,
            ConsoleLogLevel::Error => 3,
        };
        self.events
            .borrow_mut()
            .push_back(NavigationEvent::ConsoleMessage {
                level,
                message,
                source: String::new(),
                line: 0,
            });
    }
}

fn cursor_shape(cursor: Cursor) -> CursorShape {
    match cursor {
        Cursor::None => CursorShape::Custom("none".to_owned()),
        Cursor::Pointer => CursorShape::Pointer,
        Cursor::Text | Cursor::VerticalText => CursorShape::Text,
        Cursor::Wait => CursorShape::Wait,
        Cursor::Progress => CursorShape::Progress,
        Cursor::Crosshair | Cursor::Cell => CursorShape::Crosshair,
        Cursor::Move => CursorShape::Move,
        Cursor::NotAllowed | Cursor::NoDrop => CursorShape::NotAllowed,
        Cursor::Help => CursorShape::Help,
        Cursor::NResize | Cursor::SResize | Cursor::NsResize | Cursor::RowResize => {
            CursorShape::ResizeNs
        }
        Cursor::EResize | Cursor::WResize | Cursor::EwResize | Cursor::ColResize => {
            CursorShape::ResizeEw
        }
        Cursor::NeResize | Cursor::SwResize | Cursor::NeswResize => CursorShape::ResizeNeSw,
        Cursor::NwResize | Cursor::SeResize | Cursor::NwseResize => CursorShape::ResizeNwSe,
        Cursor::AllScroll => CursorShape::ResizeAll,
        Cursor::Grab => CursorShape::Grab,
        Cursor::Grabbing => CursorShape::Grabbing,
        Cursor::ZoomIn => CursorShape::ZoomIn,
        Cursor::ZoomOut => CursorShape::ZoomOut,
        _ => CursorShape::Default,
    }
}

/// Servo's rendering context: the shared colour buffers.
struct SharedContext {
    target: GlSharedTarget,
}

impl RenderingContext for SharedContext {
    fn prepare_for_rendering(&self) {
        self.target.bind_back();
    }

    fn read_to_image(&self, rect: DeviceIntRect) -> Option<RgbaImage> {
        self.target.bind_back();
        let gl = self.target.gleam();
        let (width, height) = (rect.width(), rect.height());
        let pixels = gl.read_pixels(
            rect.min.x,
            rect.min.y,
            width,
            height,
            gleam::gl::RGBA,
            gleam::gl::UNSIGNED_BYTE,
        );
        // GL rows run bottom-up.
        let row = width as usize * 4;
        let flipped = pixels.chunks_exact(row).rev().flatten().copied().collect();
        RgbaImage::from_raw(width as u32, height as u32, flipped)
    }

    fn size(&self) -> PhysicalSize<u32> {
        self.target.size()
    }

    fn resize(&self, size: PhysicalSize<u32>) {
        if let Err(error) = self.target.resize(size) {
            eprintln!("render-wgpu: the Servo page's buffers did not resize: {error}");
        }
    }

    fn present(&self) {
        self.target.finish();
    }

    fn make_current(&self) -> Result<(), surfman::Error> {
        self.target
            .make_current()
            .map_err(|_| surfman::Error::Failed)
    }

    fn gleam_gl_api(&self) -> Rc<dyn gleam::gl::Gl> {
        self.target.gleam()
    }

    fn glow_gl_api(&self) -> Arc<glow::Context> {
        self.target.glow()
    }
}

/// One page in Servo.
pub(super) struct ServoPage {
    webview: WebView,
    context: Rc<SharedContext>,
    delegate: Rc<Delegate>,
}

impl ServoPage {
    pub(super) fn new(
        runtime: &ServoRuntime,
        adapter: &wgpu::AdapterInfo,
        url: &str,
        size: PhysicalSize<u32>,
        scale_factor: f32,
    ) -> Result<Self, String> {
        let url = url::Url::parse(url).map_err(|e| format!("the UI page URL {url}: {e}"))?;
        let target =
            GlSharedTarget::new(adapter, size).map_err(|e| format!("Servo's GL target: {e}"))?;
        let context = Rc::new(SharedContext { target });
        let delegate = Rc::new(Delegate::default());
        let webview = WebViewBuilder::new(&runtime.servo, context.clone())
            .url(url)
            .hidpi_scale_factor(euclid::Scale::new(scale_factor))
            .delegate(delegate.clone())
            .build();
        webview.show();
        Ok(Self {
            webview,
            context,
            delegate,
        })
    }

    /// The GL renderer Servo draws with.
    pub(super) fn renderer(&self) -> String {
        self.context.target.renderer()
    }

    pub(super) fn resize(&self, width: u32, height: u32) {
        self.webview.resize(PhysicalSize::new(width, height));
    }

    pub(super) fn set_scale_factor(&self, scale_factor: f32) {
        self.webview
            .set_hidpi_scale_factor(euclid::Scale::new(scale_factor));
    }

    pub(super) fn focus(&self) {
        self.webview.set_focused(true);
    }

    pub(super) fn execute_script(&self, script: &str) {
        self.webview.evaluate_javascript(script, |_| {});
    }

    pub(super) fn poll_event(&self) -> Option<NavigationEvent> {
        self.delegate.events.borrow_mut().pop_front()
    }

    pub(super) fn poll_cursor(&self) -> Option<CursorShape> {
        self.delegate.cursors.borrow_mut().pop_front()
    }

    pub(super) fn send_mouse(&self, event: MouseEvent) {
        // The shell reports leaving the view as a move to (-1, -1).
        if event.x < 0 && event.y < 0 && event.action == MouseAction::Moved {
            self.webview
                .notify_input_event(InputEvent::MouseLeftViewport(MouseLeftViewportEvent {
                    focus_moving_to_another_iframe: false,
                }));
            return;
        }
        let point = WebViewPoint::Device(DevicePoint::new(event.x as f32, event.y as f32));
        let input = match event.action {
            MouseAction::Moved => InputEvent::MouseMove(MouseMoveEvent::new(point)),
            MouseAction::Pressed | MouseAction::Released => {
                let action = if event.action == MouseAction::Pressed {
                    MouseButtonAction::Down
                } else {
                    MouseButtonAction::Up
                };
                let button = match event.button {
                    MouseButton::Left => servo::MouseButton::Primary,
                    MouseButton::Middle => servo::MouseButton::Auxiliary,
                    MouseButton::Right => servo::MouseButton::Secondary,
                };
                InputEvent::MouseButton(MouseButtonEvent::new(action, button, point))
            }
            // Positive deltas scroll up and left, as the shell sends them.
            MouseAction::WheelScrolled { delta_x, delta_y } => InputEvent::Wheel(WheelEvent::new(
                WheelDelta {
                    x: f64::from(delta_x),
                    y: f64::from(delta_y),
                    z: 0.0,
                    mode: WheelMode::DeltaPixel,
                },
                point,
            )),
        };
        self.webview.notify_input_event(input);
    }

    pub(super) fn send_key(&self, input: &KeyInput<'_>) {
        let state = if input.pressed {
            KeyState::Down
        } else {
            KeyState::Up
        };
        let key = match input.key {
            " " => Key::Character(" ".into()),
            key => key.parse().unwrap_or_else(|_| Key::Character(key.into())),
        };
        let mut event = KeyboardEvent::from_state_and_key(state, key);
        event.event.code = input.code.parse().unwrap_or(Code::Unidentified);
        event.event.location = location(input.code);
        event.event.repeat = input.repeat;
        let mut modifiers = Modifiers::empty();
        modifiers.set(Modifiers::SHIFT, input.modifiers.shift);
        modifiers.set(Modifiers::CONTROL, input.modifiers.ctrl);
        modifiers.set(Modifiers::ALT, input.modifiers.alt);
        modifiers.set(Modifiers::META, input.modifiers.meta);
        event.event.modifiers = modifiers;
        self.webview.notify_input_event(InputEvent::Keyboard(event));
    }

    /// Whether Servo has a frame it has not painted yet.
    pub(super) fn frame_ready(&self) -> bool {
        self.delegate.frame_ready.get()
    }

    /// Paint Servo's newest frame into the back buffer and finish it, making
    /// it the front buffer. The caller has made sure the host no longer reads
    /// the back buffer.
    pub(super) fn paint(&self) {
        self.delegate.frame_ready.set(false);
        self.webview.paint();
        self.context.present();
    }

    pub(super) fn target(&self) -> &GlSharedTarget {
        &self.context.target
    }
}

/// `KeyboardEvent.location` for a W3C `code`.
fn location(code: &str) -> Location {
    if code.starts_with("Numpad") {
        Location::Numpad
    } else if code.ends_with("Left") && !code.starts_with("Arrow") {
        Location::Left
    } else if code.ends_with("Right") && !code.starts_with("Arrow") {
        Location::Right
    } else {
        Location::Standard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_preferences_take_their_type_from_the_text() {
        assert!(matches!(preference_value("true"), PrefValue::Bool(true)));
        assert!(matches!(preference_value("12"), PrefValue::Int(12)));
        assert!(matches!(preference_value("0.5"), PrefValue::Float(v) if v == 0.5));
        assert!(matches!(preference_value("en-GB"), PrefValue::Str(s) if s == "en-GB"));
    }

    #[test]
    fn modifier_and_numpad_codes_carry_their_location() {
        assert_eq!(location("ShiftLeft"), Location::Left);
        assert_eq!(location("ControlRight"), Location::Right);
        assert_eq!(location("Numpad7"), Location::Numpad);
        assert_eq!(location("ArrowLeft"), Location::Standard);
        assert_eq!(location("KeyW"), Location::Standard);
    }
}
