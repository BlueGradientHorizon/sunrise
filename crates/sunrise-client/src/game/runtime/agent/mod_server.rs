use super::Game;

impl Game<'_, crate::game::server_side::ServerState> {
    #[inline(always)]
    pub(in crate::game) fn shield_cooldown(&self) -> u32 {
        self.side.shield_cooldown
    }
}
