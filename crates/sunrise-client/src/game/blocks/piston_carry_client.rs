#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, GameLevel};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_carry::CARRIED_KEY;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_carry::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{block_entities::Carried, save::ChunkExtras};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Direction, nbt::Tag, position::BlockPos,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_mod_runtime::attachments::{
    ATTACHMENTS_KEY, Attachments, attachment_compound,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{level::MovingPiston, registry::BlockStateId};

impl Game<'_> {
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
