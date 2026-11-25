//! Common command-block and command-minecart helpers shared by server and client.
use super::{
    Game,
    minecarts::{CartKind, Minecart},
};
use sunrise_client::{command_block::CommandBlock, containers::BlockEntity};
use sunrise_gameplay::command_blocks::state_of;
use sunrise_world::chunk::BlockState;

pub(in crate::game) type Pos = (i32, i32, i32);

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// `Player.canUseGameMasterBlocks`: creative with game master commands.
    pub(in crate::game) fn can_use_game_master_blocks(&self) -> bool {
        self.creative && self.ui.registries.permission >= 2
    }

    pub(in crate::game) fn command_block(&self, pos: Pos) -> Option<&CommandBlock> {
        match self.world.block_entities.get(&pos)? {
            BlockEntity::CommandBlock(c) => Some(c),
            _ => None,
        }
    }

    pub(in crate::game) fn block_state_at(&self, pos: Pos) -> Option<&'static BlockState> {
        let registry = self.pack.assets.registry;
        state_of(registry, self.world.block(pos.0, pos.1, pos.2))
    }

    pub(in crate::game) fn edit_command_cart<R>(
        &mut self,
        id: u32,
        edit: impl FnOnce(&mut Minecart) -> R,
    ) -> Option<R> {
        let i = self
            .carts
            .iter()
            .position(|c| c.id == id && c.kind == CartKind::CommandBlock)?;
        Some(self.carts.update(i, edit))
    }
}
