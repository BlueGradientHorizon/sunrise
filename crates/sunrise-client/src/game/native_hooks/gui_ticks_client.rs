use super::*;

impl Game<'_, crate::game::client_side::ClientState> {
    /// `Hud.getGuiTicks`, independent of the server and visual clocks.
    pub(in crate::game) fn gui_ticks(&self) -> i32 {
        self.side.gui_ticks
    }
}
