use super::Game;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Retires the local player body and associated residency/client state.
    pub(in crate::game) fn remove_local_player_body(&mut self) {
        self.side_remove_local_player_body();
    }
}
