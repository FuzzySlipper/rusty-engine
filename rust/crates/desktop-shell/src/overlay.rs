//! The UI page over the scene: Chromium, window input into the page, and the
//! shell-provided pointer lock.

use std::sync::Arc;

use render_wgpu::web::{
    CursorShape, EventModifiers, KeyEvent, KeyEventKind, MouseAction, MouseButton, MouseEvent,
    NavigationEvent, WebOverlay, WebOverlayStats, WebRuntime, WebRuntimeConfig,
};
use render_wgpu::Gpu;
use winit::{
    event::{ElementState, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::{CursorGrabMode, CursorIcon, Window},
};

use crate::keys;

/// Console lines the pointer-lock shim writes; the shell reads them from the
/// page's console events.
const LOCK_REQUEST: &str = "rusty-desktop:pointer-lock";
const LOCK_EXIT: &str = "rusty-desktop:pointer-unlock";

/// Chromium's off-screen mode rejects the Pointer Lock API, so the page gets
/// a shell-backed one: `requestPointerLock` asks the shell to grab the
/// cursor, `document.pointerLockElement` reports the element, and the shell
/// feeds raw motion back through `__rustyDesktopMotion`.
///
/// The off-screen page also keeps Chromium's focus while the native window is
/// in the background, so the shell reports window focus through
/// `__rustyDesktopFocus`: the page gets `blur`/`focus` on `window` and
/// `document.hasFocus()` follows the native window, as in a browser tab.
const PAGE_SHIM: &str = r#"(() => {
  if (window.__rustyDesktopMotion) return;
  let locked = null;
  let windowFocused = true;
  const hasFocus = Document.prototype.hasFocus;
  Document.prototype.hasFocus = function () {
    return windowFocused && hasFocus.call(this);
  };
  window.__rustyDesktopFocus = (focused) => {
    if (windowFocused === focused) return;
    windowFocused = focused;
    window.dispatchEvent(new FocusEvent(focused ? 'focus' : 'blur'));
  };
  const change = () => document.dispatchEvent(new Event('pointerlockchange'));
  Object.defineProperty(Document.prototype, 'pointerLockElement', {
    configurable: true,
    get() { return locked; },
  });
  Element.prototype.requestPointerLock = function () {
    if (locked !== this) {
      locked = this;
      console.info('rusty-desktop:pointer-lock');
      change();
    }
    return Promise.resolve();
  };
  Document.prototype.exitPointerLock = function () {
    if (locked === null) return;
    locked = null;
    console.info('rusty-desktop:pointer-unlock');
    change();
  };
  window.__rustyDesktopUnlocked = () => {
    if (locked === null) return;
    locked = null;
    change();
  };
  window.__rustyDesktopMotion = (x, y) => {
    if (locked === null) return;
    locked.dispatchEvent(new PointerEvent('pointermove', {
      bubbles: true, composed: true, pointerType: 'mouse', isPrimary: true,
      movementX: x, movementY: y,
    }));
  };
})();"#;

/// Physical pixels per wheel line, as Chromium scrolls on Linux.
const WHEEL_PIXELS_PER_LINE: f64 = 53.0;

pub(crate) struct UiOverlay {
    // The page goes before the runtime that hosts it.
    page: WebOverlay,
    runtime: WebRuntime,
    cursor: (i32, i32),
    modifiers: ModifiersState,
    buttons: [bool; 3],
    grabbed: bool,
    focused: bool,
    motion: (f64, f64),
}

impl UiOverlay {
    pub(crate) fn open(
        config: &WebRuntimeConfig,
        gpu: &Gpu,
        url: &str,
        window: &Arc<Window>,
    ) -> Result<Self, String> {
        let runtime = WebRuntime::initialize(config.clone())?;
        let size = window.inner_size();
        let mut page = WebOverlay::new(
            &runtime,
            gpu,
            url,
            size.width,
            size.height,
            window.scale_factor() as f32,
        )?;
        page.focus()?;
        Ok(Self {
            page,
            runtime,
            cursor: (0, 0),
            modifiers: ModifiersState::empty(),
            buttons: [false; 3],
            grabbed: false,
            focused: window.has_focus(),
            motion: (0.0, 0.0),
        })
    }

    pub(crate) fn page(&mut self) -> &mut WebOverlay {
        &mut self.page
    }

    pub(crate) fn stats(&self) -> WebOverlayStats {
        self.page.stats()
    }

    pub(crate) fn last_error(&self) -> Option<String> {
        self.page.last_error().map(str::to_owned)
    }

    /// Chromium's work, the page's requests, and this frame's locked motion.
    pub(crate) fn pump(&mut self, window: &Window) {
        self.runtime.pump();
        while let Some(event) = self.page.poll_event() {
            match event {
                NavigationEvent::LoadStart { .. } | NavigationEvent::LoadEnd { .. } => {
                    // Before the page's scripts ask for a lock, and again
                    // after, since a load start may still see the old page.
                    let _ = self.page.execute_script(PAGE_SHIM);
                    if !self.focused {
                        self.report_focus();
                    }
                    if matches!(event, NavigationEvent::LoadStart { .. }) {
                        self.release(window, false);
                    }
                }
                NavigationEvent::ConsoleMessage { message, .. } if message == LOCK_REQUEST => {
                    self.grab(window);
                }
                NavigationEvent::ConsoleMessage { message, .. } if message == LOCK_EXIT => {
                    self.release(window, false);
                }
                _ => {}
            }
        }
        while let Some(shape) = self.page.poll_cursor() {
            if !self.grabbed {
                window.set_cursor(cursor_icon(&shape));
            }
        }
        let (x, y) = std::mem::take(&mut self.motion);
        if self.grabbed && (x != 0.0 || y != 0.0) {
            let _ = self
                .page
                .execute_script(&format!("window.__rustyDesktopMotion({x}, {y});"));
        }
    }

    pub(crate) fn raw_motion(&mut self, x: f64, y: f64) {
        if self.grabbed {
            self.motion.0 += x;
            self.motion.1 += y;
        }
    }

    fn grab(&mut self, window: &Window) {
        let grabbed = window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
        if grabbed.is_err() {
            // No grab on this platform: tell the page the lock is gone.
            let _ = self.page.execute_script("window.__rustyDesktopUnlocked();");
            return;
        }
        window.set_cursor_visible(false);
        self.grabbed = true;
        self.motion = (0.0, 0.0);
    }

    /// Release the cursor. `tell_page` when the shell, not the page, ended
    /// the lock (Escape, focus loss), as a browser reports an ended lock.
    fn release(&mut self, window: &Window, tell_page: bool) {
        if !self.grabbed {
            return;
        }
        let _ = window.set_cursor_grab(CursorGrabMode::None);
        window.set_cursor_visible(true);
        self.grabbed = false;
        if tell_page {
            let _ = self.page.execute_script("window.__rustyDesktopUnlocked();");
        }
    }

    /// Tell the page whether the native window has focus.
    fn report_focus(&mut self) {
        let _ = self
            .page
            .execute_script(&format!("window.__rustyDesktopFocus({});", self.focused));
    }

    pub(crate) fn window_event(&mut self, window: &Window, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                let _ = self.page.resize(size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let _ = self.page.set_scale_factor(*scale_factor as f32);
            }
            WindowEvent::Focused(false) => {
                self.release(window, true);
                // Buttons and modifiers released while another window has
                // focus never reach this one; the page clears its own held
                // input on the blur.
                self.buttons = [false; 3];
                self.modifiers = ModifiersState::empty();
                self.focused = false;
                self.report_focus();
            }
            WindowEvent::Focused(true) => {
                self.focused = true;
                let _ = self.page.focus();
                self.report_focus();
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as i32, position.y as i32);
                if !self.grabbed {
                    self.mouse(MouseButton::Left, MouseAction::Moved);
                }
            }
            WindowEvent::CursorLeft { .. } if !self.grabbed => {
                // Chromium treats a move outside the view as leaving it. Keep
                // the last position: a pointer that re-enters without moving
                // (a window mapped under it) reports no new one.
                let inside = self.cursor;
                self.cursor = (-1, -1);
                self.mouse(MouseButton::Left, MouseAction::Moved);
                self.cursor = inside;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let (button, index) = match button {
                    winit::event::MouseButton::Left => (MouseButton::Left, 0),
                    winit::event::MouseButton::Middle => (MouseButton::Middle, 1),
                    winit::event::MouseButton::Right => (MouseButton::Right, 2),
                    _ => return,
                };
                let pressed = *state == ElementState::Pressed;
                self.buttons[index] = pressed;
                let action = if pressed {
                    MouseAction::Pressed
                } else {
                    MouseAction::Released
                };
                self.mouse(button, action);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (
                        f64::from(*x) * WHEEL_PIXELS_PER_LINE,
                        f64::from(*y) * WHEEL_PIXELS_PER_LINE,
                    ),
                    MouseScrollDelta::PixelDelta(position) => (position.x, position.y),
                };
                self.mouse(
                    MouseButton::Left,
                    MouseAction::WheelScrolled {
                        delta_x: x.round() as i32,
                        delta_y: y.round() as i32,
                    },
                );
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                let pressed = event.state == ElementState::Pressed;
                // As in a browser, Escape ends a pointer lock and the page
                // sees the lock end, not the key.
                if code == KeyCode::Escape && self.grabbed {
                    if pressed {
                        self.release(window, true);
                    }
                    return;
                }
                let Some(windows_key_code) = keys::windows_key_code(code) else {
                    return;
                };
                let native_key_code = keys::native_key_code(code);
                let modifiers = self.event_modifiers();
                let kind = if pressed {
                    KeyEventKind::RawKeyDown
                } else {
                    KeyEventKind::KeyUp
                };
                let _ = self.page.send_key(KeyEvent {
                    kind,
                    windows_key_code,
                    native_key_code,
                    character: None,
                    modifiers,
                });
                if pressed && !self.modifiers.control_key() && !self.modifiers.super_key() {
                    for character in event.text.iter().flat_map(|text| text.chars()) {
                        if character.is_control() {
                            continue;
                        }
                        let _ = self.page.send_key(KeyEvent {
                            kind: KeyEventKind::Char,
                            windows_key_code: character as i32,
                            native_key_code,
                            character: Some(character),
                            modifiers,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    fn mouse(&mut self, button: MouseButton, action: MouseAction) {
        let (x, y) = self.cursor;
        let modifiers = self.event_modifiers();
        let _ = self.page.send_mouse(MouseEvent {
            x,
            y,
            button,
            action,
            modifiers,
        });
    }

    fn event_modifiers(&self) -> EventModifiers {
        EventModifiers {
            shift: self.modifiers.shift_key(),
            ctrl: self.modifiers.control_key(),
            alt: self.modifiers.alt_key(),
            meta: self.modifiers.super_key(),
            left_mouse_button: self.buttons[0],
            middle_mouse_button: self.buttons[1],
            right_mouse_button: self.buttons[2],
        }
    }
}

fn cursor_icon(shape: &CursorShape) -> CursorIcon {
    match shape {
        CursorShape::Pointer => CursorIcon::Pointer,
        CursorShape::Text => CursorIcon::Text,
        CursorShape::Wait => CursorIcon::Wait,
        CursorShape::Crosshair => CursorIcon::Crosshair,
        CursorShape::Move => CursorIcon::Move,
        CursorShape::NotAllowed => CursorIcon::NotAllowed,
        CursorShape::Help => CursorIcon::Help,
        CursorShape::Progress => CursorIcon::Progress,
        CursorShape::ResizeNs => CursorIcon::NsResize,
        CursorShape::ResizeEw => CursorIcon::EwResize,
        CursorShape::ResizeNeSw => CursorIcon::NeswResize,
        CursorShape::ResizeNwSe => CursorIcon::NwseResize,
        CursorShape::ResizeAll => CursorIcon::AllScroll,
        CursorShape::Grab => CursorIcon::Grab,
        CursorShape::Grabbing => CursorIcon::Grabbing,
        CursorShape::ZoomIn => CursorIcon::ZoomIn,
        CursorShape::ZoomOut => CursorIcon::ZoomOut,
        _ => CursorIcon::Default,
    }
}
