//! `ClientViewClientHost::set_overlay_message`: a client mod's action-bar line,
//! written to the one overlay slot the game's own lines use (`Hud.setOverlayMessage`).
use super::*;

impl Game<'_> {
    /// `Hud.setOverlayMessage(line, animate_color)` on this window's HUD, outside a borrowed
    /// server guest's turn: no chat, narration or packet, as the direct setter does none.
    pub(in crate::game) fn set_overlay_message(
        &mut self,
        line: sunrise_client::text::Text,
        animate_color: bool,
    ) {
        if self.native_window() && self.lan_acting.is_none() {
            self.titles.set_overlay(line, animate_color);
        }
    }
}
