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
    AllowOrDenyRequest, ClipboardDelegate, Code, ConsoleLogLevel, Cursor, DeviceIntRect,
    DevicePoint, EmbedderControl, EmbedderControlId, EventLoopWaker, GamepadEvent, GamepadIndex,
    GamepadInputBounds, GamepadSupportedHapticEffects, GamepadUpdateType, InputEvent, Key,
    KeyState, KeyboardEvent, LoadStatus, Location, Modifiers, MouseButtonAction, MouseButtonEvent,
    MouseLeftViewportEvent, MouseMoveEvent, PrefValue, RenderingContext, RgbaImage, SelectElement,
    SelectElementOptionOrOptgroup, Servo, ServoBuilder, ServoDelegate, StringRequest,
    UserContentManager, UserScript, WebView, WebViewBuilder, WebViewDelegate, WebViewPoint,
    WheelDelta, WheelEvent, WheelMode,
};
use welding::{GlSharedTarget, PopupRect};

use super::{
    CursorShape, KeyInput, MouseAction, MouseButton, MouseEvent, NavigationEvent, PageControl,
    PageOption,
};

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
        let mut prefs = servo::Preferences {
            // The page is composited over the scene: what it leaves undrawn
            // stays transparent.
            shell_background_color_rgba: [0.0, 0.0, 0.0, 0.0],
            // `navigator.clipboard`, as Chromium has it.
            dom_async_clipboard_enabled: true,
            ..Default::default()
        };
        // Before Servo starts: some (the devtools server's) only take effect
        // then.
        for (name, text) in preferences {
            let value = preference_value(&prefs, name, text)?;
            prefs.set_value(name, value);
        }
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
        servo.set_delegate(Rc::new(RuntimeDelegate));
        Ok(Self { servo })
    }

    pub(super) fn pump(&self) {
        self.servo.spin_event_loop();
    }
}

/// Preference `name`'s value as typed on a command line, of the type its
/// current value has. Servo panics on an unknown name or a value of another
/// type, so those are refused here.
fn preference_value(
    prefs: &servo::Preferences,
    name: &str,
    text: &str,
) -> Result<PrefValue, String> {
    if !servo::Preferences::exists(name) {
        return Err(format!("Servo has no preference {name}"));
    }
    let wrong = |kind: &str| format!("Servo's preference {name} takes {kind}, not {text}");
    Ok(match prefs.get_value(name) {
        PrefValue::Bool(_) => PrefValue::Bool(text.parse().map_err(|_| wrong("true or false"))?),
        PrefValue::Int(_) => PrefValue::Int(text.parse().map_err(|_| wrong("an integer"))?),
        PrefValue::UInt(_) => PrefValue::UInt(text.parse().map_err(|_| wrong("a whole number"))?),
        PrefValue::Float(_) => PrefValue::Float(text.parse().map_err(|_| wrong("a number"))?),
        PrefValue::Str(_) => PrefValue::Str(text.to_owned()),
        PrefValue::Array(_) => return Err(format!("Servo's preference {name} is a list")),
    })
}

/// Servo's process-wide callbacks. A developer starts its devtools server
/// (`devtools_server_enabled`, on 127.0.0.1 by default); its connections are
/// theirs, so they are allowed.
struct RuntimeDelegate;

impl ServoDelegate for RuntimeDelegate {
    fn notify_devtools_server_started(&self, port: u16, _token: String) {
        eprintln!("render-wgpu: Servo's devtools server listens on port {port}");
    }

    fn request_devtools_connection(&self, request: AllowOrDenyRequest) {
        request.allow();
    }
}

/// The page writes the system clipboard and never reads it, as in Chromium,
/// where the read's permission prompt is denied.
#[derive(Default)]
struct PageClipboard {
    /// Kept: on Linux the copied text lives as long as its owner.
    clipboard: RefCell<Option<arboard::Clipboard>>,
}

impl ClipboardDelegate for PageClipboard {
    fn get_text(&self, _: WebView, request: StringRequest) {
        request.failure("the desktop window denies clipboard reads".to_owned());
    }

    fn set_text(&self, _: WebView, text: String) {
        let mut clipboard = self.clipboard.borrow_mut();
        if clipboard.is_none() {
            *clipboard = arboard::Clipboard::new().ok();
        }
        if let Some(clipboard) = clipboard.as_mut() {
            let _ = clipboard.set_text(text);
        }
    }
}

/// What Servo told the page since the shell last asked.
#[derive(Default)]
struct Delegate {
    frame_ready: Cell<bool>,
    events: RefCell<VecDeque<NavigationEvent>>,
    cursors: RefCell<VecDeque<CursorShape>>,
    controls: RefCell<VecDeque<PageControl>>,
    /// The `<select>` whose option list the shell is drawing.
    select: RefCell<Option<SelectElement>>,
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

    /// The shell draws a `<select>`'s option list. Other controls a browser
    /// shows natively (file and colour pickers, context menus, dialogs) are
    /// dropped, which cancels them, as off-screen Chromium shows none.
    fn show_embedder_control(&self, _: WebView, control: EmbedderControl) {
        let EmbedderControl::SelectElement(select) = control else {
            return;
        };
        if select.allow_select_multiple() {
            return;
        }
        let selected = select.selected_options();
        let mut options = Vec::new();
        let mut add = |option: &servo::SelectElementOption, group: Option<&str>| {
            options.push(PageOption {
                id: option.id,
                label: option.label.clone(),
                disabled: option.is_disabled,
                selected: selected.contains(&option.id),
                group: group.map(str::to_owned),
            });
        };
        for entry in select.options() {
            match entry {
                SelectElementOptionOrOptgroup::Option(option) => add(option, None),
                SelectElementOptionOrOptgroup::Optgroup { label, options } => {
                    for option in options {
                        add(option, Some(label));
                    }
                }
            }
        }
        let rect = select.position();
        self.controls.borrow_mut().push_back(PageControl::Select {
            rect: PopupRect {
                x: rect.min.x,
                y: rect.min.y,
                width: rect.width().max(0) as u32,
                height: rect.height().max(0) as u32,
            },
            options,
        });
        *self.select.borrow_mut() = Some(select);
    }

    fn hide_embedder_control(&self, _: WebView, id: EmbedderControlId) {
        let mut select = self.select.borrow_mut();
        if select.as_ref().is_some_and(|select| select.id() == id) {
            select.take();
            self.controls.borrow_mut().push_back(PageControl::Closed);
        }
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
    /// The gamepad the page last heard of: sticks, then button values.
    gamepad: RefCell<Option<([f32; 4], [f32; 16])>>,
}

impl ServoPage {
    /// `document_script` runs at the start of every document the page loads.
    pub(super) fn new(
        runtime: &ServoRuntime,
        host: &welding::HostWgpuContext,
        adapter: &wgpu::AdapterInfo,
        url: &str,
        size: PhysicalSize<u32>,
        scale_factor: f32,
        document_script: Option<&str>,
    ) -> Result<Self, String> {
        let url = url::Url::parse(url).map_err(|e| format!("the UI page URL {url}: {e}"))?;
        let target = GlSharedTarget::new(host, adapter, size)
            .map_err(|e| format!("Servo's GL target: {e}"))?;
        let context = Rc::new(SharedContext { target });
        let delegate = Rc::new(Delegate::default());
        let contents = Rc::new(UserContentManager::new(&runtime.servo));
        if let Some(script) = document_script {
            contents.add_script(Rc::new(UserScript::new(script.to_owned(), None)));
        }
        let webview = WebViewBuilder::new(&runtime.servo, context.clone())
            .url(url)
            .hidpi_scale_factor(euclid::Scale::new(scale_factor))
            .delegate(delegate.clone())
            .clipboard_delegate(Rc::new(PageClipboard::default()))
            .user_content_manager(contents)
            .build();
        webview.show();
        Ok(Self {
            webview,
            context,
            delegate,
            gamepad: RefCell::new(None),
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

    pub(super) fn poll_control(&self) -> Option<PageControl> {
        self.delegate.controls.borrow_mut().pop_front()
    }

    /// Choose option `id` of the `<select>` whose list the shell drew, or
    /// keep its choice with `None`, and close the list.
    pub(super) fn answer_select(&self, id: Option<usize>) {
        let Some(mut select) = self.delegate.select.borrow_mut().take() else {
            return;
        };
        if let Some(id) = id {
            select.select(vec![id]);
        }
        select.submit();
    }

    /// The first gamepad's state in the standard mapping: Servo reads no pads
    /// itself, so the page's `navigator.getGamepads()` shows this one.
    pub(super) fn send_gamepad(&self, axes: [f32; 4], values: [f32; 16]) {
        let index = GamepadIndex(0);
        let mut last = self.gamepad.borrow_mut();
        let previous = match *last {
            Some(previous) => previous,
            None => {
                self.webview
                    .notify_input_event(InputEvent::Gamepad(GamepadEvent::Connected(
                        index,
                        "Standard gamepad".to_owned(),
                        GamepadInputBounds {
                            axis_bounds: (-1.0, 1.0),
                            button_bounds: (0.0, 1.0),
                        },
                        GamepadSupportedHapticEffects {
                            supports_dual_rumble: false,
                            supports_trigger_rumble: false,
                        },
                    )));
                ([f32::NAN; 4], [f32::NAN; 16])
            }
        };
        let changes = axes
            .iter()
            .zip(previous.0)
            .enumerate()
            .filter(|(_, (now, before))| **now != *before)
            .map(|(axis, (now, _))| GamepadUpdateType::Axis(axis, f64::from(*now)))
            .chain(
                values
                    .iter()
                    .zip(previous.1)
                    .enumerate()
                    .filter(|(_, (now, before))| **now != *before)
                    .map(|(button, (now, _))| GamepadUpdateType::Button(button, f64::from(*now))),
            );
        for change in changes {
            self.webview
                .notify_input_event(InputEvent::Gamepad(GamepadEvent::Updated(index, change)));
        }
        *last = Some((axes, values));
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
    fn command_line_preferences_take_the_preference_s_own_type() {
        let prefs = servo::Preferences::default();
        let value = |name, text| preference_value(&prefs, name, text);
        assert!(matches!(
            value("devtools_server_enabled", "true"),
            Ok(PrefValue::Bool(true))
        ));
        assert!(matches!(
            value("devtools_server_listen_address", "127.0.0.1:7011"),
            Ok(PrefValue::Str(address)) if address == "127.0.0.1:7011"
        ));
        assert!(value("devtools_server_enabled", "7011").is_err());
        assert!(value("no_such_preference", "true").is_err());
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
