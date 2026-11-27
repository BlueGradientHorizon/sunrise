use sunrise_client::pvp::ChatOptions;

#[derive(Default)]
pub(in crate::game) struct PvpState {
    /// While the chat box holds the search query (Ctrl+F), its message text and draft state.
    pub(in crate::game) chat_search_aside: Option<(String, bool)>,
}
impl crate::game::ClientGame<'_> {
    /// The chat's options: Ctrl+F search, copy on click, and the rest (`ChatOptions::qol`).
    pub(in crate::game::client_side) fn pvp_chat(&self) -> ChatOptions {
        self.client.settings.extras.pvp.chat
    }
}
