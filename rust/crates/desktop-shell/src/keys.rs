//! Window key events in the form Chromium's off-screen input expects: a
//! Windows virtual-key code for `key`/`keyCode` and the platform's native key
//! code, from which Chromium derives `KeyboardEvent.code`. The browser
//! shell's input capture reads `code`, so the native code is what keeps
//! gameplay input identical to the browser.

use winit::keyboard::KeyCode;
use winit::platform::scancode::PhysicalKeyExtScancode;

/// The Windows virtual-key code Chromium uses for `keyCode` and key
/// identity, or `None` for keys the page never sees.
pub(crate) fn windows_key_code(code: KeyCode) -> Option<i32> {
    use KeyCode::*;
    let letter = |offset: u8| Some(i32::from(b'A' + offset));
    let digit = |offset: u8| Some(i32::from(b'0' + offset));
    Some(match code {
        KeyA => return letter(0),
        KeyB => return letter(1),
        KeyC => return letter(2),
        KeyD => return letter(3),
        KeyE => return letter(4),
        KeyF => return letter(5),
        KeyG => return letter(6),
        KeyH => return letter(7),
        KeyI => return letter(8),
        KeyJ => return letter(9),
        KeyK => return letter(10),
        KeyL => return letter(11),
        KeyM => return letter(12),
        KeyN => return letter(13),
        KeyO => return letter(14),
        KeyP => return letter(15),
        KeyQ => return letter(16),
        KeyR => return letter(17),
        KeyS => return letter(18),
        KeyT => return letter(19),
        KeyU => return letter(20),
        KeyV => return letter(21),
        KeyW => return letter(22),
        KeyX => return letter(23),
        KeyY => return letter(24),
        KeyZ => return letter(25),
        Digit0 => return digit(0),
        Digit1 => return digit(1),
        Digit2 => return digit(2),
        Digit3 => return digit(3),
        Digit4 => return digit(4),
        Digit5 => return digit(5),
        Digit6 => return digit(6),
        Digit7 => return digit(7),
        Digit8 => return digit(8),
        Digit9 => return digit(9),
        Numpad0 => 0x60,
        Numpad1 => 0x61,
        Numpad2 => 0x62,
        Numpad3 => 0x63,
        Numpad4 => 0x64,
        Numpad5 => 0x65,
        Numpad6 => 0x66,
        Numpad7 => 0x67,
        Numpad8 => 0x68,
        Numpad9 => 0x69,
        NumpadMultiply => 0x6A,
        NumpadAdd => 0x6B,
        NumpadSubtract => 0x6D,
        NumpadDecimal => 0x6E,
        NumpadDivide => 0x6F,
        NumpadEnter => 0x0D,
        F1 => 0x70,
        F2 => 0x71,
        F3 => 0x72,
        F4 => 0x73,
        F5 => 0x74,
        F6 => 0x75,
        F7 => 0x76,
        F8 => 0x77,
        F9 => 0x78,
        F10 => 0x79,
        F11 => 0x7A,
        F12 => 0x7B,
        Backspace => 0x08,
        Tab => 0x09,
        Enter => 0x0D,
        ShiftLeft | ShiftRight => 0x10,
        ControlLeft | ControlRight => 0x11,
        AltLeft | AltRight => 0x12,
        Pause => 0x13,
        CapsLock => 0x14,
        Escape => 0x1B,
        Space => 0x20,
        PageUp => 0x21,
        PageDown => 0x22,
        End => 0x23,
        Home => 0x24,
        ArrowLeft => 0x25,
        ArrowUp => 0x26,
        ArrowRight => 0x27,
        ArrowDown => 0x28,
        PrintScreen => 0x2C,
        Insert => 0x2D,
        Delete => 0x2E,
        SuperLeft => 0x5B,
        SuperRight => 0x5C,
        ContextMenu => 0x5D,
        NumLock => 0x90,
        ScrollLock => 0x91,
        Semicolon => 0xBA,
        Equal => 0xBB,
        Comma => 0xBC,
        Minus => 0xBD,
        Period => 0xBE,
        Slash => 0xBF,
        Backquote => 0xC0,
        BracketLeft => 0xDB,
        Backslash => 0xDC,
        BracketRight => 0xDD,
        Quote => 0xDE,
        IntlBackslash => 0xE2,
        _ => return None,
    })
}

/// The native key code Chromium maps to `KeyboardEvent.code`: the XKB
/// keycode on Linux, the scan code on Windows, the virtual key
/// code on macOS.
pub(crate) fn native_key_code(code: KeyCode) -> i32 {
    let Some(scancode) = code.to_scancode() else {
        return 0;
    };
    if cfg!(target_os = "linux") {
        // winit reports the evdev code; XKB keycodes are evdev + 8.
        scancode as i32 + 8
    } else if cfg!(windows) {
        // Chromium reads the scan code, 0xE0-prefixed for extended keys,
        // which is winit's scancode as is.
        scancode as i32
    } else {
        scancode as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gameplay_keys_have_virtual_codes() {
        for code in [
            KeyCode::KeyW,
            KeyCode::KeyA,
            KeyCode::KeyS,
            KeyCode::KeyD,
            KeyCode::Space,
            KeyCode::ControlLeft,
            KeyCode::ShiftLeft,
            KeyCode::Escape,
            KeyCode::Enter,
            KeyCode::ArrowUp,
            KeyCode::Digit1,
            KeyCode::Tab,
            KeyCode::Backquote,
        ] {
            assert!(windows_key_code(code).is_some(), "{code:?}");
        }
        assert_eq!(windows_key_code(KeyCode::KeyW), Some(0x57));
        assert_eq!(windows_key_code(KeyCode::Digit1), Some(0x31));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_native_codes_are_xkb_keycodes() {
        // evdev KEY_W is 17, so its XKB keycode is 25.
        assert_eq!(native_key_code(KeyCode::KeyW), 25);
        assert_eq!(native_key_code(KeyCode::Escape), 9);
    }

    #[cfg(windows)]
    #[test]
    fn windows_native_codes_are_scan_codes() {
        // Chromium maps these to KeyboardEvent.code; an lParam gives "".
        assert_eq!(native_key_code(KeyCode::KeyW), 0x11);
        assert_eq!(native_key_code(KeyCode::ArrowUp), 0xE048);
    }
}
