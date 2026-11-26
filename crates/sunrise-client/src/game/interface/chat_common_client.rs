use super::Game;

impl Game<'_, crate::game::client_side::ClientState> {
    /// `Component.translatable`: the selected language, falling back to `fallback`.
    pub(in crate::game) fn translate_with_fallback(&self, key: &str, fallback: &str) -> String {
        sunrise_client_core::lang::get_or(key, fallback)
    }
}
