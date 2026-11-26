use super::Game;
use sunrise_client_core::text::Text;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `ServerPlayer.sendOverlayMessage`: queue an action-bar `SystemChat` packet.
    pub(in crate::game) fn server_send_overlay_message(&mut self, message: Text) {
        assert!(
            self.is_logical_server(),
            "server_send_overlay_message called on Client role"
        );
        self.side
            .guest_packets
            .push(sunrise_protocol::packets::play::Clientbound::SystemChat(
                sunrise_protocol::packets::play::SystemChat {
                    content: sunrise_sim::lan::data::component_nbt(&message.component()),
                    overlay: true,
                },
            ));
    }
}
