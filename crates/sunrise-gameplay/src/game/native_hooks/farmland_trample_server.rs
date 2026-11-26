use super::*;
use sunrise_world::block_landing::FarmlandTrample;

/// The hook point. It does nothing in Sunrise.
impl crate::game::ServerGame<'_> {
    /// Apply the owning-host decision inside `FarmlandBlock.fallOn`, while the
    /// landing is paused. The caller resumes fall damage even when this refuses.
    pub(in crate::game::server_side) fn trample_farmland(
        &mut self,
        entity: u32,
        player: bool,
        trample: FarmlandTrample,
    ) {
        let FarmlandTrample {
            position: pos,
            state,
            base,
            distance,
            ..
        } = trample;
        let at = [pos.x, pos.y, pos.z];
        let dimension = self.common.dim;
        let still = |game: &Self| {
            game.dim == dimension
                && !game.side.world_debug
                && game.world.is_loaded_at(pos.x, pos.y, pos.z)
                && !game.world.carrier_fences.locks_cell(&at)
                && game.world.block(pos.x, pos.y, pos.z) == state
        };
        if !still(self) {
            return;
        }
        let applied = if mod_api::__private::listened(mod_api::HookId::FarmlandTrample) {
            let Some(living) = self.common.entity(entity).and_then(|e| self.common.living(e)) else {
                return;
            };
            self.storage_transaction(|game, _| {
                let mut cancel = false;
                game.native_farmland_trample(living, pos, state, distance, &mut cancel);
                let valid =
                    |game: &Self| game.entity(entity) == Some(living.entity()) && still(game);
                (!cancel && game.apply_farmland_trample(player, trample, valid, true))
                    .then_some(())
                    .ok_or(())
            })
            .is_ok()
        } else {
            self.apply_farmland_trample(player, trample, still, false)
        };
        if applied {
            let who = if player {
                Who::Player
            } else {
                Who::Mob(entity)
            };
            self.after_commit(sunrise_gameplay_common::game::transactions::After::BlockGameEvent(
                "minecraft:block_change",
                at,
                who,
                base,
            ));
        }
    }
}

use sunrise_gameplay_common::game::transactions::TransactionsGameExt as _;
