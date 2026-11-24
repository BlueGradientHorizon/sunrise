#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::Loan;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::ResidentBlockEntityAccess;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::ServerBlockEntityData;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::valid_tick_subject;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_tick::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{
    Dim,
    api_values::{TryIntoApi, TryIntoEngine},
    transactions::OpenTransactions,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    BlockEntityAccess, BlockEntityItemError, BlockEntityRef, BlockEntityScope, BlockEntityView,
    ResidentBlockEntity, ServerHost, TransactionContext, TransactionScopeError,
    TransactionUnavailable,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::HashMap;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    block_entities::BlockEntityLoan, chunk_map::ChunkMap, fast_hash::FxHashMap,
    world::ModifiedChunks,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::{Compound, JavaString, Tag};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_items::items::Items;

impl BlockEntityAccess for ResidentBlockEntityAccess<'_, '_> {
    fn with_view(&mut self, call: &mut dyn FnMut(BlockEntityView<'_>)) {
        use mod_api::__private::game::Fields;
        let Some(subject) = self.subject else { return };
        let target = subject.fields();
        let [x, y, z] = target.position;
        if !self.game.level_in_place(target.level) || !self.game.world.loaded(x, z) {
            return;
        }
        let live_state = self.game.world.block(x, y, z);
        if matches!(self.loan, Loan::Menu(admitted) if admitted != live_state) {
            return;
        }
        let Ok(state) = live_state.try_into_api() else {
            return;
        };
        let slot_count = self
            .game
            .world
            .block_entities
            .mod_container(live_state.0)
            .map_or(0, |c| c.slots);
        let pos = (x, y, z);
        let Some(loan) = self
            .game
            .world
            .block_entities
            .loan_mod_entity(pos, target.generation)
        else {
            return;
        };
        if let Loan::Tick(checked) = &mut self.loan
            && *checked != Some(live_state)
        {
            if !valid_tick_subject(&self.game.res.facts().get(live_state).name, loan.entity) {
                return;
            }
            *checked = Some(live_state);
        }
        let mut data = ServerBlockEntityData {
            dim: self.game.dim,
            pos,
            loan,
            slot_count,
            items: self.game.res.items(),
            journal: &mut self.game.transactions,
            chunks: &self.game.world.chunks,
            names: &self.game.world.block_names,
            hoppers: &self.game.server.hoppers,
            menu: matches!(self.loan, Loan::Menu(_))
                .then_some((&mut self.game.world.modified, &mut self.items_changed)),
            dirty: false,
        };
        call(BlockEntityView::new(subject, state, &mut data));
    }

    fn host(&mut self) -> &mut dyn ServerHost {
        self.game
    }

    fn with_current_transaction_scope(
        &mut self,
        callback: &mut dyn for<'loan> FnMut(
            &'loan mut dyn BlockEntityAccess,
            &'loan TransactionContext,
        ),
    ) -> Result<(), TransactionUnavailable> {
        let frame = self
            .game
            .current_storage_frame()
            .ok_or(TransactionUnavailable)?;
        frame.lend(|tx| callback(self, tx));
        Ok(())
    }

    fn with_transaction_scope(
        &mut self,
        call: &mut dyn for<'loan> FnMut(
            &'loan mut dyn BlockEntityAccess,
            &'loan TransactionContext,
        ) -> bool,
    ) -> Result<bool, TransactionScopeError> {
        let (subject, loan) = (self.subject, self.loan);
        let committed = self.game.scoped_storage(|game, tx| {
            let mut child = ResidentBlockEntityAccess::new(game, subject, loan);
            call(&mut child, tx).then_some(child.items_changed)
        })?;
        if let Some(changed) = committed {
            self.items_changed |= changed;
        }
        Ok(committed.is_some())
    }
}

#[cfg(test)]
#[path = "block_entity_tick/tests_server.rs"]
pub(in crate::game) mod tests;
