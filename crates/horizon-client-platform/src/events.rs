//! Window and pointing-device events consumed by Horizon's shared client logic.
//!
//! Frontends translate their native events once at ingress. The game handlers
//! operate on these values so desktop, CafeOS, and replay input share a path.

use crate::{KeyState, KeyboardEvent, Modifiers, MouseButton};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicalPosition<T> {
    pub x: T,
    pub y: T,
}

impl<T> PhysicalPosition<T> {
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PhysicalSize<T> {
    pub width: T,
    pub height: T,
}

impl<T> PhysicalSize<T> {
    pub const fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MouseScrollDelta {
    LineDelta(f32, f32),
    PixelDelta(PhysicalPosition<f64>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImeEvent {
    Enabled,
    Preedit(String, Option<(usize, usize)>),
    Commit(String),
    Disabled,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClientWindowEvent {
    KeyboardInput {
        event: KeyboardEvent,
        is_synthetic: bool,
    },
    ModifiersChanged(Modifiers),
    MouseInput {
        state: KeyState,
        button: MouseButton,
    },
    CursorMoved {
        position: PhysicalPosition<f64>,
    },
    MouseWheel {
        delta: MouseScrollDelta,
        phase: TouchPhase,
    },
    Focused(bool),
    Resized(PhysicalSize<u32>),
    CloseRequested,
    RedrawRequested,
    Ime(ImeEvent),
    DroppedFile(PathBuf),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClientDeviceEvent {
    MouseMotion { delta: (f64, f64) },
}
