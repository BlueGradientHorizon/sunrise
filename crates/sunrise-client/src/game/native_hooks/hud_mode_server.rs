#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::hud_mode::*;

impl Game<'_> {
    /// Set HUD and hand visibility outside a borrowed server guest's turn.
    pub(in crate::game) fn set_hud_mode(&mut self, mode: mod_api::HudMode) {
        if self.native_window() && self.server.lan_acting.is_none() {
            self.ui.set_hud_mode(mode);
        }
    }
}
