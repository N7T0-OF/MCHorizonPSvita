//! CafeOS input adapter for Horizon's existing controller processing.
//!
//! The host feeds the returned `PadState` to Horizon; bindings, button edges,
//! deadzones, menu navigation and Nintendo letter swapping remain in Horizon.

use horizon_client_input::gamepad::{Button, PadState};

/// The scalar subset read from WUT's `VPADStatus` by `vpad_bridge.c`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VpadSample {
    pub buttons: u32,
    pub left_x: f32,
    pub left_y: f32,
    pub right_x: f32,
    pub right_y: f32,
}

/// Convert WUT VPAD button positions and stick axes to Horizon's pad state.
pub fn pad_state(sample: VpadSample) -> PadState {
    let mut state = PadState {
        left: [sample.left_x, sample.left_y],
        right: [sample.right_x, sample.right_y],
        triggers: [
            if sample.buttons & 0x0080 != 0 {
                1.0
            } else {
                0.0
            },
            if sample.buttons & 0x0040 != 0 {
                1.0
            } else {
                0.0
            },
        ],
        ..PadState::default()
    };
    // WUT's VPAD_BUTTON_* values; Horizon identifies face buttons by position.
    for (mask, button) in [
        (0x4000, Button::South), // B
        (0x8000, Button::East),  // A
        (0x1000, Button::West),  // Y
        (0x2000, Button::North), // X
        (0x0020, Button::LeftBumper),
        (0x0010, Button::RightBumper),
        (0x0004, Button::Select),
        (0x0008, Button::Start),
        (0x0004_0000, Button::LeftStick),
        (0x0002_0000, Button::RightStick),
        (0x0200, Button::DPadUp),
        (0x0100, Button::DPadDown),
        (0x0800, Button::DPadLeft),
        (0x0400, Button::DPadRight),
    ] {
        state.set(button, sample.buttons & mask != 0);
    }
    state
}

/// Retains the last state when VPAD has no new sample in a frame.
#[derive(Default)]
pub struct VpadState(PadState);

impl VpadState {
    pub fn update(&mut self, sample: Option<VpadSample>) -> PadState {
        if let Some(sample) = sample {
            self.0 = pad_state(sample);
        }
        self.0
    }

    /// Clear held controls when the host loses the controller.
    pub fn reset(&mut self) -> PadState {
        self.0 = PadState::default();
        self.0
    }

    /// Read VPAD channel 0. On a non-busy error the host must feed a neutral
    /// state to Horizon; the cached state is cleared before returning the error.
    #[cfg(all(feature = "cafe", target_arch = "powerpc"))]
    pub fn poll(&mut self) -> Result<PadState, i32> {
        extern "C" {
            fn horizon_wiiu_vpad_read(sample: *mut VpadSample) -> i32;
        }
        let mut sample = VpadSample::default();
        // The C bridge writes the sample only on success and owns VPADStatus's ABI.
        match unsafe { horizon_wiiu_vpad_read(&mut sample) } {
            1 => Ok(self.update(Some(sample))),
            -1 => Ok(self.update(None)),
            // A busy channel can be retried without releasing held controls.
            -4 => Err(-4),
            error => {
                self.reset();
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_wii_u_controls_to_horizons_positional_pad() {
        let state = pad_state(VpadSample {
            buttons: 0x4000 | 0x8000 | 0x0020 | 0x0040 | 0x0200 | 0x0002_0000,
            left_x: -0.5,
            left_y: 0.75,
            right_x: 0.25,
            right_y: -1.0,
        });
        assert!(state.held(Button::South));
        assert!(state.held(Button::East));
        assert!(state.held(Button::LeftBumper));
        assert!(state.held(Button::DPadUp));
        assert!(state.held(Button::RightStick));
        assert!(!state.held(Button::North));
        assert_eq!(state.triggers, [0.0, 1.0]);
        assert_eq!(state.left, [-0.5, 0.75]);
        assert_eq!(state.right, [0.25, -1.0]);
    }

    #[test]
    fn no_new_sample_keeps_buttons_held_until_a_release_sample() {
        let mut reader = VpadState::default();
        assert!(reader
            .update(Some(VpadSample {
                buttons: 0x4000,
                ..Default::default()
            }))
            .held(Button::South));
        assert!(reader.update(None).held(Button::South));
        assert!(!reader
            .update(Some(VpadSample::default()))
            .held(Button::South));
    }

    #[test]
    fn reset_releases_a_previously_held_button() {
        let mut reader = VpadState::default();
        reader.update(Some(VpadSample {
            buttons: 0x4000,
            ..Default::default()
        }));
        assert!(!reader.reset().held(Button::South));
        assert!(!reader.update(None).held(Button::South));
    }
}
