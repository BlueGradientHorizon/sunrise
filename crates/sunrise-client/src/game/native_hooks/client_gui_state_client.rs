#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::client_gui_state::*;

impl Game<'_, crate::game::client_side::ClientState> {
    /// The same screen identifier provided to screen hooks.
    pub(in crate::game) fn observed_screen_id(&self) -> Option<ScreenId> {
        self.observed_screen_view().map(|view| ScreenId {
            id: view.id.to_owned(),
        })
    }
    /// The last window focus state reported by the window system.
    pub(in crate::game) fn window_focused(&self) -> bool {
        self.side.focused
    }
}
