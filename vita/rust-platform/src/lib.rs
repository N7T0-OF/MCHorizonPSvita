#![no_std]
//! Capability contract only. It does not render Minecraft or perform downloads.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability { Available, NeedsPort, NotSupported }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub renderer: Availability,
    pub input: Availability,
    pub native_mods: Availability,
    pub shaders: Availability,
    pub java_mod_jars: Availability,
    pub gameplay: Availability,
}

impl Capabilities {
    pub const fn stage_zero() -> Self {
        Self {
            renderer: Availability::NeedsPort,
            input: Availability::Available, // provided only by the separate C/SDL2 bootstrap
            native_mods: Availability::NeedsPort,
            shaders: Availability::NeedsPort,
            java_mod_jars: Availability::NotSupported,
            gameplay: Availability::NeedsPort,
        }
    }
    pub const fn can_start_horizon(&self) -> bool {
        matches!(self.renderer, Availability::Available)
            && matches!(self.gameplay, Availability::Available)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_false_claim_of_playable_engine() {
        assert!(!Capabilities::stage_zero().can_start_horizon());
        assert_eq!(Capabilities::stage_zero().java_mod_jars, Availability::NotSupported);
    }
}
