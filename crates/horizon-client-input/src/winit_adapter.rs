//! Desktop input ingress/egress. The game-facing key identities live in
//! `horizon-client-platform`; Winit is translated here at the window boundary.

use horizon_client_platform::{KeyCode, KeyState, KeyboardEvent, Modifiers, MouseButton};
use winit::{
    event::{ElementState, KeyEvent as WinitKeyEvent, MouseButton as WinitMouseButton},
    keyboard::{Key as WinitKey, KeyCode as WinitKeyCode, PhysicalKey},
};

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

/// Convert a desktop key event at the window boundary into Horizon's shared
/// keyboard payload.
pub fn from_winit_key_event(event: &WinitKeyEvent) -> KeyboardEvent {
    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
    let unmodified_key = event.key_without_modifiers();
    from_winit_key_values(
        event.physical_key,
        event.state,
        &event.logical_key,
        event.text.as_deref(),
        &unmodified_key,
        event.repeat,
    )
}

fn from_winit_key_values(
    physical_key: PhysicalKey,
    state: ElementState,
    logical_key: &WinitKey,
    text: Option<&str>,
    unmodified_key: &WinitKey,
    repeat: bool,
) -> KeyboardEvent {
    let logical_text = match logical_key {
        WinitKey::Character(text) => Some(text.to_string()),
        _ => None,
    };
    let unmodified_text = match unmodified_key {
        WinitKey::Character(text) => Some(text.to_string()),
        WinitKey::Dead(Some(character)) => Some(character.to_string()),
        _ => None,
    };
    KeyboardEvent {
        physical_key: match physical_key {
            PhysicalKey::Code(code) => Some(from_winit_key(code).unwrap_or(KeyCode::Other)),
            PhysicalKey::Unidentified(_) => None,
        },
        state: match state {
            ElementState::Pressed => KeyState::Pressed,
            ElementState::Released => KeyState::Released,
        },
        logical_text,
        text: text.map(str::to_owned),
        unmodified_text,
        repeat,
    }
}

/// Translate a Winit key event's logical text and physical position to the
/// platform-neutral edit-shortcut letter used by Horizon.
pub fn shortcut_letter_from_winit(logical: &WinitKey, physical: PhysicalKey) -> Option<char> {
    let logical_text = match logical {
        WinitKey::Character(text) => Some(text.as_ref()),
        _ => None,
    };
    let physical = match physical {
        PhysicalKey::Code(code) => from_winit_key(code).unwrap_or(KeyCode::Other),
        PhysicalKey::Unidentified(_) => KeyCode::Other,
    };
    crate::clipboard::shortcut_letter(physical, logical_text)
}

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
    fn shortcut_mapping_prefers_logical_text_then_falls_back_to_position() {
        use winit::keyboard::Key;

        assert_eq!(
            shortcut_letter_from_winit(
                &Key::Character("a".into()),
                PhysicalKey::Code(WinitKeyCode::KeyQ),
            ),
            Some('a')
        );
        assert_eq!(
            shortcut_letter_from_winit(
                &Key::Character("с".into()),
                PhysicalKey::Code(WinitKeyCode::KeyC),
            ),
            Some('c')
        );
        assert_eq!(
            shortcut_letter_from_winit(
                &Key::Unidentified(winit::keyboard::NativeKey::Unidentified),
                PhysicalKey::Code(WinitKeyCode::KeyV),
            ),
            Some('v')
        );
    }

    #[test]
    fn keyboard_event_conversion_keeps_text_sources_and_key_state_distinct() {
        let input = from_winit_key_values(
            PhysicalKey::Code(WinitKeyCode::KeyA),
            ElementState::Pressed,
            &WinitKey::Character("á".into()),
            Some("áé"),
            &WinitKey::Character("a".into()),
            true,
        );
        assert_eq!(
            input,
            KeyboardEvent {
                physical_key: Some(KeyCode::KeyA),
                state: KeyState::Pressed,
                logical_text: Some("á".to_owned()),
                text: Some("áé".to_owned()),
                unmodified_text: Some("a".to_owned()),
                repeat: true,
            }
        );

        let release = from_winit_key_values(
            PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified),
            ElementState::Released,
            &WinitKey::Named(winit::keyboard::NamedKey::Enter),
            None,
            &WinitKey::Dead(None),
            false,
        );
        assert_eq!(release.physical_key, None);
        assert_eq!(release.state, KeyState::Released);
        assert_eq!(release.text, None);
        assert_eq!(release.unmodified_text, None);
        assert!(!release.repeat);
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
