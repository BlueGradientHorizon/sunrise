#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::gui_ticks::*;

impl Game<'_> {
    /// `Hud.getGuiTicks`, independent of the server and visual clocks.
    pub(in crate::game) fn gui_ticks(&self) -> i32 {
        self.client.gui_ticks
    }
}
