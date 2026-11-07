//! Desktop input ingress/egress. The game-facing key identities live in
//! `horizon-client-platform`; Winit is translated here at the window boundary.

use horizon_client_platform::{
    ClientDeviceEvent, ClientWindowEvent, ImeEvent, KeyCode, KeyLocation, KeyState, KeyboardEvent,
    LogicalKey, Modifiers, MouseButton, MouseScrollDelta, PhysicalPosition, PhysicalSize,
    TouchPhase,
};
use winit::{
    event::{
        DeviceEvent as WinitDeviceEvent, ElementState, Ime as WinitIme, KeyEvent as WinitKeyEvent,
        MouseButton as WinitMouseButton, MouseScrollDelta as WinitMouseScrollDelta,
        TouchPhase as WinitTouchPhase, WindowEvent as WinitWindowEvent,
    },
    keyboard::{
        Key as WinitKey, KeyCode as WinitKeyCode, KeyLocation as WinitKeyLocation,
        NamedKey as WinitNamedKey, PhysicalKey,
    },
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
        event.location,
        event.text.as_deref(),
        &unmodified_key,
        event.repeat,
    )
}

fn from_winit_key_values(
    physical_key: PhysicalKey,
    state: ElementState,
    logical_key: &WinitKey,
    location: WinitKeyLocation,
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
        logical_key: from_winit_logical_key(logical_key),
        location: match location {
            WinitKeyLocation::Standard => KeyLocation::Standard,
            WinitKeyLocation::Left => KeyLocation::Left,
            WinitKeyLocation::Right => KeyLocation::Right,
            WinitKeyLocation::Numpad => KeyLocation::Numpad,
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

fn from_winit_logical_key(key: &WinitKey) -> LogicalKey {
    match key {
        WinitKey::Character(text) => LogicalKey::Character(text.to_string()),
        WinitKey::Named(WinitNamedKey::Escape) => LogicalKey::Named("Escape".to_owned()),
        WinitKey::Named(WinitNamedKey::CapsLock) => LogicalKey::Named("CapsLock".to_owned()),
        WinitKey::Named(named) => LogicalKey::Named(format!("{named:?}")),
        WinitKey::Dead(character) => LogicalKey::Dead(*character),
        WinitKey::Unidentified(_) => LogicalKey::Unidentified,
    }
}

/// Convert window events consumed by Horizon's game into platform-neutral values.
/// Unused Winit events return `None` and remain at the desktop window boundary.
pub fn from_winit_window_event(event: &WinitWindowEvent) -> Option<ClientWindowEvent> {
    Some(match event {
        WinitWindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } => ClientWindowEvent::KeyboardInput {
            event: from_winit_key_event(event),
            is_synthetic: *is_synthetic,
        },
        WinitWindowEvent::ModifiersChanged(modifiers) => {
            ClientWindowEvent::ModifiersChanged(from_winit_modifiers(modifiers.state()))
        }
        WinitWindowEvent::MouseInput { state, button, .. } => ClientWindowEvent::MouseInput {
            state: from_winit_state(*state),
            button: from_winit_mouse(*button),
        },
        WinitWindowEvent::CursorMoved { position, .. } => ClientWindowEvent::CursorMoved {
            position: PhysicalPosition::new(position.x, position.y),
        },
        WinitWindowEvent::MouseWheel { delta, phase, .. } => ClientWindowEvent::MouseWheel {
            delta: match delta {
                WinitMouseScrollDelta::LineDelta(x, y) => MouseScrollDelta::LineDelta(*x, *y),
                WinitMouseScrollDelta::PixelDelta(position) => {
                    MouseScrollDelta::PixelDelta(PhysicalPosition::new(position.x, position.y))
                }
            },
            phase: from_winit_touch_phase(*phase),
        },
        WinitWindowEvent::Focused(focused) => ClientWindowEvent::Focused(*focused),
        WinitWindowEvent::Resized(size) => {
            ClientWindowEvent::Resized(PhysicalSize::new(size.width, size.height))
        }
        WinitWindowEvent::CloseRequested => ClientWindowEvent::CloseRequested,
        WinitWindowEvent::RedrawRequested => ClientWindowEvent::RedrawRequested,
        WinitWindowEvent::Ime(event) => ClientWindowEvent::Ime(match event {
            WinitIme::Enabled => ImeEvent::Enabled,
            WinitIme::Preedit(text, cursor) => ImeEvent::Preedit(text.clone(), *cursor),
            WinitIme::Commit(text) => ImeEvent::Commit(text.clone()),
            WinitIme::Disabled => ImeEvent::Disabled,
        }),
        WinitWindowEvent::DroppedFile(path) => ClientWindowEvent::DroppedFile(path.clone()),
        _ => return None,
    })
}

/// Convert only relative pointer motion; other device events are not consumed by Game.
pub fn from_winit_device_event(event: &WinitDeviceEvent) -> Option<ClientDeviceEvent> {
    match event {
        WinitDeviceEvent::MouseMotion { delta } => {
            Some(ClientDeviceEvent::MouseMotion { delta: *delta })
        }
        _ => None,
    }
}

fn from_winit_state(state: ElementState) -> KeyState {
    match state {
        ElementState::Pressed => KeyState::Pressed,
        ElementState::Released => KeyState::Released,
    }
}

fn from_winit_touch_phase(phase: WinitTouchPhase) -> TouchPhase {
    match phase {
        WinitTouchPhase::Started => TouchPhase::Started,
        WinitTouchPhase::Moved => TouchPhase::Moved,
        WinitTouchPhase::Ended => TouchPhase::Ended,
        WinitTouchPhase::Cancelled => TouchPhase::Cancelled,
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
            WinitKeyLocation::Standard,
            Some("áé"),
            &WinitKey::Character("a".into()),
            true,
        );
        assert_eq!(
            input,
            KeyboardEvent {
                physical_key: Some(KeyCode::KeyA),
                logical_key: LogicalKey::Character("á".to_owned()),
                location: KeyLocation::Standard,
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
            WinitKeyLocation::Numpad,
            None,
            &WinitKey::Dead(None),
            false,
        );
        assert_eq!(release.physical_key, None);
        assert_eq!(release.logical_key, LogicalKey::Named("Enter".to_owned()));
        assert_eq!(release.location, KeyLocation::Numpad);
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

    #[test]
    fn window_event_conversion_preserves_mouse_ime_resize_and_focus_data() {
        let cursor = WinitWindowEvent::CursorMoved {
            device_id: winit::event::DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(12.5, 38.25),
        };
        assert_eq!(
            from_winit_window_event(&cursor),
            Some(ClientWindowEvent::CursorMoved {
                position: PhysicalPosition::new(12.5, 38.25),
            })
        );

        let wheel = WinitWindowEvent::MouseWheel {
            device_id: winit::event::DeviceId::dummy(),
            delta: WinitMouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(2.0, -30.0)),
            phase: WinitTouchPhase::Moved,
        };
        assert_eq!(
            from_winit_window_event(&wheel),
            Some(ClientWindowEvent::MouseWheel {
                delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(2.0, -30.0)),
                phase: TouchPhase::Moved,
            })
        );

        assert_eq!(
            from_winit_window_event(&WinitWindowEvent::Ime(WinitIme::Commit("text".to_owned()))),
            Some(ClientWindowEvent::Ime(ImeEvent::Commit("text".to_owned())))
        );
        assert_eq!(
            from_winit_window_event(&WinitWindowEvent::Resized(winit::dpi::PhysicalSize::new(
                1280, 720
            ),)),
            Some(ClientWindowEvent::Resized(PhysicalSize::new(1280, 720)))
        );
        assert_eq!(
            from_winit_window_event(&WinitWindowEvent::Focused(false)),
            Some(ClientWindowEvent::Focused(false))
        );
        assert!(from_winit_window_event(&WinitWindowEvent::Occluded(false)).is_none());
    }

    #[test]
    fn device_motion_conversion_keeps_relative_deltas_and_ignores_other_events() {
        assert_eq!(
            from_winit_device_event(&WinitDeviceEvent::MouseMotion { delta: (1.5, -3.0) }),
            Some(ClientDeviceEvent::MouseMotion { delta: (1.5, -3.0) })
        );
        assert!(from_winit_device_event(&WinitDeviceEvent::Added).is_none());
    }
}
