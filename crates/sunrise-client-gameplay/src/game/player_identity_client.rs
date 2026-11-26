use crate::game::ClientGame;

impl ClientGame<'_> {
    /// Retires the local player body and associated residency/client state.
    pub(in crate::game) fn remove_local_player_body(&mut self) {
        self.side_remove_local_player_body();
    }
}

use sunrise_gameplay_common::game::side_ops::player::PlayerSideGameExt as _;
