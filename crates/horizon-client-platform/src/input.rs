//! Platform-neutral physical input identities.
//!
//! These identify physical controls, not translated characters. A window
//! backend converts its native key/button values at the input boundary; a
//! CafeOS backend can map WPAD/KPAD controls to the same IDs.

use std::ops::BitOr;

/// Modifier keys held by the platform when an input event was received.
///
/// This intentionally contains only the four modifiers used by Horizon's
/// shared input logic. Backends translate their native modifier state at the
/// event boundary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const SHIFT: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(Self::SHIFT.0 | Self::CONTROL.0 | Self::ALT.0 | Self::SUPER.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn set(&mut self, modifier: Self, value: bool) {
        if value {
            self.0 |= modifier.0;
        } else {
            self.0 &= !modifier.0;
        }
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn shift_key(self) -> bool {
        self.contains(Self::SHIFT)
    }

    pub const fn control_key(self) -> bool {
        self.contains(Self::CONTROL)
    }

    pub const fn alt_key(self) -> bool {
        self.contains(Self::ALT)
    }

    pub const fn super_key(self) -> bool {
        self.contains(Self::SUPER)
    }
}

impl BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// Physical keyboard keys used by Horizon's key mappings.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum KeyCode {
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
    /// A physical key unknown to this platform's shared key set.
    Other,
}

/// Mouse buttons with the same one-based numbering used by `key.mouse.N`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}
