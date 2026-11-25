use super::Game;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn server_update_spawner_display(&mut self, _key: (i32, i32, i32)) -> bool {
        false
    }

    /// `PlayerList.isOp` for the player acting now: a server's guest by their
    /// own level, else the window's player.
    pub(in crate::game) fn acting_player_is_op(&self) -> bool {
        self.side
            .lan_acting_permission
            .unwrap_or(self.ui.registries.permission)
            > 0
    }
}
