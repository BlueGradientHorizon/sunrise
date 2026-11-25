use super::Game;
use sunrise_core::position::BlockPos;

impl Game<'_, crate::game::client_side::ClientState> {
    /// A client's moving piston at `at` once the server's block took its
    /// place (`LevelChunk.setBlockState`), with the block entity its copy
    /// carried: the server sends its own (`BlockEntityData`).
    pub(in crate::game::client_side) fn forget_moving_piston(&mut self, at: BlockPos) {
        self.block_ticks.moving_pistons.retain(|(p, _)| *p != at);
        self.world
            .block_entities
            .discard_carried(&(at.x, at.y, at.z));
    }
}
