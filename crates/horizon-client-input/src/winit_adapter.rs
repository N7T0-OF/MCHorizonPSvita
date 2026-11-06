//! Desktop input ingress/egress. The game-facing key identities live in
//! `horizon-client-platform`; Winit is translated here at the window boundary.

use horizon_client_platform::{KeyCode, Modifiers, MouseButton};
use winit::{event::MouseButton as WinitMouseButton, keyboard::KeyCode as WinitKeyCode};

macro_rules! key_codes {
    ($($key:ident),+ $(,)?) => {
        pub fn from_winit_key(key: WinitKeyCode) -> Option<KeyCode> {
            Some(match key {
                $(WinitKeyCode::$key => KeyCode::$key,)+
                _ => return None,
            })
        }

        pub fn to_winit_key(key: KeyCode) -> Option<WinitKeyCode> {
            Some(match key {
                $(KeyCode::$key => WinitKeyCode::$key,)+
                KeyCode::Other => return None,
            })
        }
    };
}

key_codes!(
    Again,
    AltLeft,
    AltRight,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Backquote,
    Backslash,
    Backspace,
    BracketLeft,
    BracketRight,
    CapsLock,
    Comma,
    ContextMenu,
    ControlLeft,
    ControlRight,
    Copy,
    Cut,
    Delete,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    End,
    Enter,
    Equal,
    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    Find,
    Help,
    Home,
    Insert,
    IntlBackslash,
    IntlRo,
    IntlYen,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Lang1,
    Lang2,
    Lang3,
    Lang4,
    Lang5,
    Minus,
    NumLock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadBackspace,
    NumpadClear,
    NumpadClearEntry,
    NumpadComma,
    NumpadDecimal,
    NumpadDivide,
    NumpadEnter,
    NumpadEqual,
    NumpadHash,
    NumpadMemoryAdd,
    NumpadMemoryClear,
    NumpadMemoryRecall,
    NumpadMemoryStore,
    NumpadMemorySubtract,
    NumpadMultiply,
    NumpadParenLeft,
    NumpadParenRight,
    NumpadSubtract,
    PageDown,
    PageUp,
    Paste,
    Pause,
    Period,
    PrintScreen,
    Quote,
    ScrollLock,
    Select,
    Semicolon,
    ShiftLeft,
    ShiftRight,
    Slash,
    Space,
    SuperLeft,
    SuperRight,
    Tab,
    Undo,
);

pub fn from_winit_mouse(button: WinitMouseButton) -> MouseButton {
    match button {
        WinitMouseButton::Left => MouseButton::Left,
        WinitMouseButton::Right => MouseButton::Right,
        WinitMouseButton::Middle => MouseButton::Middle,
        WinitMouseButton::Back => MouseButton::Back,
        WinitMouseButton::Forward => MouseButton::Forward,
        WinitMouseButton::Other(number) => MouseButton::Other(number),
    }
}

pub fn to_winit_mouse(button: MouseButton) -> WinitMouseButton {
    match button {
        MouseButton::Left => WinitMouseButton::Left,
        MouseButton::Right => WinitMouseButton::Right,
        MouseButton::Middle => WinitMouseButton::Middle,
        MouseButton::Back => WinitMouseButton::Back,
        MouseButton::Forward => WinitMouseButton::Forward,
        MouseButton::Other(number) => WinitMouseButton::Other(number),
    }
}

pub fn from_winit_modifiers(modifiers: winit::keyboard::ModifiersState) -> Modifiers {
    let mut result = Modifiers::empty();
    result.set(Modifiers::SHIFT, modifiers.shift_key());
    result.set(Modifiers::CONTROL, modifiers.control_key());
    result.set(Modifiers::ALT, modifiers.alt_key());
    result.set(Modifiers::SUPER, modifiers.super_key());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_mapping_preserves_physical_identity_and_unknown_fallback() {
        assert_eq!(from_winit_key(WinitKeyCode::KeyW), Some(KeyCode::KeyW));
        assert_eq!(
            to_winit_key(KeyCode::ArrowLeft),
            Some(WinitKeyCode::ArrowLeft)
        );
        assert_eq!(from_winit_key(WinitKeyCode::Fn), None);
    }

    #[test]
    fn mouse_mapping_preserves_other_button_number() {
        assert_eq!(
            from_winit_mouse(WinitMouseButton::Other(6)),
            MouseButton::Other(6)
        );
        assert_eq!(
            to_winit_mouse(MouseButton::Other(6)),
            WinitMouseButton::Other(6)
        );
    }

    #[test]
    fn modifier_mapping_preserves_the_four_shared_modifier_bits() {
        use winit::keyboard::ModifiersState as WinitModifiers;

        for (native, shared) in [
            (WinitModifiers::empty(), Modifiers::empty()),
            (WinitModifiers::SHIFT, Modifiers::SHIFT),
            (WinitModifiers::CONTROL, Modifiers::CONTROL),
            (WinitModifiers::ALT, Modifiers::ALT),
            (WinitModifiers::SUPER, Modifiers::SUPER),
            (
                WinitModifiers::SHIFT | WinitModifiers::ALT,
                Modifiers::SHIFT | Modifiers::ALT,
            ),
            (
                WinitModifiers::CONTROL | WinitModifiers::SUPER,
                Modifiers::CONTROL | Modifiers::SUPER,
            ),
            (WinitModifiers::all(), Modifiers::all()),
        ] {
            assert_eq!(from_winit_modifiers(native), shared, "{native:?}");
        }
    }
}
