use super::Game;

impl Game<'_, crate::game::client_side::ClientState> {
    /// The window's frame step: changed config files reload once a second.
    pub(in crate::game) fn native_configs_tick_client(&mut self) {
        self.native_configs_poll();
    }
}
