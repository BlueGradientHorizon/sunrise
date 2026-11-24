#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::titles::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::hud_overlays::Titles;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    hud::HudBatch, hud_layout, hud_overlays::HeldName, text::Text,
};

impl Game<'_> {
    /// `ServerPlayer.sendOverlayMessage`: the steady player sees it on this
    /// game's HUD; a server client receives it as a packet.
    pub(in crate::game) fn send_overlay_message(&mut self, message: Text) {
        if self.server.lan_acting.is_some() {
            self.server.guest_packets.push(
                sunrise_protocol::packets::play::Clientbound::SystemChat(
                    sunrise_protocol::packets::play::SystemChat {
                        content: sunrise_client::lan::data::component_nbt(&message.component()),
                        overlay: true,
                    },
                ),
            );
        } else if self.options.dedicated.is_none() {
            self.set_hud_overlay(message);
        }
    }
}
