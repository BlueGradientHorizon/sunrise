#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_breaking::*;

impl Game<'_> {
    /// `ServerPlayerGameMode.destroyBlock`: whether the block was removed.
    /// A break inside another (a mod breaking more blocks from its
    /// `native_destroy_block`) keeps the outer break's own events.
    pub(in crate::game) fn player_break_block(&mut self, pos: [i32; 3]) -> Result<bool> {
        let outer = self.server.lan_break_pos.replace(pos);
        let broken = self.with_player_actor(|game| game.break_block_as_player(pos));
        self.server.lan_break_pos = outer;
        broken
    }
}
