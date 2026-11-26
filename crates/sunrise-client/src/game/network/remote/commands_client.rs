use super::*;
use sunrise_protocol::packets::play::CommandSuggestions;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// `ClientPacketListener.handleCommandSuggestions`: forward the reply to the command edit box.
    pub(in crate::game) fn handle_command_suggestions(&mut self, packet: CommandSuggestions) {
        let lang = self
            .side
            .remote
            .as_ref()
            .map(|r| r.lang.clone())
            .unwrap_or_default();
        let list: Vec<_> = packet
            .suggestions
            .into_iter()
            .map(|entry| {
                (
                    entry.text,
                    entry.tooltip.as_ref().map(|tag| plain(tag, &lang)),
                )
            })
            .collect();
        let range = (packet.start.max(0) as usize, packet.length.max(0) as usize);
        self.chat_server_suggestions(packet.id, range, list.clone());
        self.command_block_server_suggestions(packet.id, range, list);
    }
}
