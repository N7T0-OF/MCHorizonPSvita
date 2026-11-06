//! What the game asks of the loop that runs it.
//!
//! A window's game runs inside winit's event loop, and its methods used to take winit's
//! `ActiveEventLoop` only to stop the loop or to say when to wake next. A headless client
//! (#4951) has no display, so it cannot have that loop: it runs its own, and hands the same
//! methods this trait instead. winit's loop implements it, so a window's game is unchanged.

pub(super) use horizon_client_platform::{EventLoopControl, EventLoopCtl, redraw_flow};
use winit::event_loop::{ActiveEventLoop, ControlFlow};

/// Adapts Winit's event loop to the platform-neutral scheduler contract.
pub(super) struct WinitEventLoop<'a>(&'a ActiveEventLoop);

impl WinitEventLoop<'_> {
    pub(super) fn new(event_loop: &ActiveEventLoop) -> WinitEventLoop<'_> {
        WinitEventLoop(event_loop)
    }
}

impl EventLoopCtl for WinitEventLoop<'_> {
    fn set_control_flow(&self, control: EventLoopControl) {
        match control {
            EventLoopControl::Exit => ActiveEventLoop::exit(self.0),
            EventLoopControl::Poll => ActiveEventLoop::set_control_flow(self.0, ControlFlow::Poll),
            EventLoopControl::WaitUntil(at) => {
                ActiveEventLoop::set_control_flow(self.0, ControlFlow::WaitUntil(at));
            }
        }
    }
}
