#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::client_gui_state::*;

impl Game<'_> {
    /// The last window focus state reported by the window system.
    pub(in crate::game) fn window_focused(&self) -> bool {
        self.client.focused
    }
}
