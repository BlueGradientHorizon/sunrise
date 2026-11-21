use super::*;
use crate::game::{
    Dim,
    api_values::{TryIntoApi, TryIntoEngine},
    transactions::OpenTransactions,
};
use mod_api::{
    BlockEntityAccess, BlockEntityDataError, BlockEntityItemError, BlockEntityRef,
    BlockEntityScope, BlockEntityView, ResidentBlockEntity, ServerHost, Transaction,
};
use std::collections::HashMap;
use sunrise_client::{
    block_entities::BlockEntityLoan, chunk_map::ChunkMap, fast_hash::FxHashMap,
    world::ModifiedChunks,
};
use sunrise_core::nbt::{Compound, JavaString, Tag};
use sunrise_items::items::Items;

#[cfg(test)]
#[path = "block_entity_tick/tests.rs"]
mod tests;

impl Game<'_> {
    /// Resolve the handle and block state for a live resident mod block entity at `pos`.
    pub(in crate::game) fn resident_block_entity_target(
        &self,
        pos: Pos,
    ) -> Option<(BlockEntityRef, BlockStateId)> {
        if !self.world.loaded(pos.0, pos.2) {
            return None;
        }
        let Some(BlockEntity::Mod(_)) = self.world.block_entities.get(&pos) else {
            return None;
        };
        let level = self.level(self.dim.id())?;
        let generation = self.world.block_entities.generation(&pos)?;
        let state = self.world.block(pos.0, pos.1, pos.2);
        let handle =
            mod_api::__private::game::block_entity_ref(level, [pos.0, pos.1, pos.2], generation);
        Some((handle, state))
    }

    /// The tick of the resident mod block entity at `pos`.
    pub(in crate::game) fn tick_mod_block_entity_loaded(&mut self, pos: Pos) {
        let Some(BlockEntity::Mod(entity)) = self.world.block_entities.get(&pos) else {
            return;
        };
        let Some(kind) = entity.tick_kind() else {
            return;
        };
        let Some((handle, state)) = self.resident_block_entity_target(pos) else {
            return;
        };
        let mut access = ResidentBlockEntityAccess::for_handle(self, handle, state);
        Self::native_block_entity_tick(kind, BlockEntityScope::new(&mut access));
    }

    /// Dispatch the one resident server ticker; the scope owns access to the Host.
    pub(in crate::game) fn native_block_entity_tick(
        kind: mod_api::BlockEntityTypeKey,
        subject: mod_api::BlockEntityScope<'_>,
    ) {
        Self::fire_block_entity_tick(kind, subject);
    }
}

pub(in crate::game) struct ResidentBlockEntityAccess<'a, 'g> {
    game: &'a mut Game<'g>,
    target: Option<(BlockEntityRef, BlockStateId)>,
    items_changed: bool,
}

impl<'a, 'g> ResidentBlockEntityAccess<'a, 'g> {
    pub(in crate::game) fn for_handle(
        game: &'a mut Game<'g>,
        handle: BlockEntityRef,
        state: BlockStateId,
    ) -> Self {
        Self {
            game,
            target: Some((handle, state)),
            items_changed: false,
        }
    }

    pub(in crate::game) fn blockless(game: &'a mut Game<'g>) -> Self {
        Self {
            game,
            target: None,
            items_changed: false,
        }
    }

    pub(in crate::game) fn items_changed(&self) -> bool {
        self.items_changed
    }
}

impl BlockEntityAccess for ResidentBlockEntityAccess<'_, '_> {
    fn with_view(&mut self, call: &mut dyn FnMut(BlockEntityView<'_>)) {
        use mod_api::__private::game::Fields;
        let Some((handle, state_id)) = self.target else {
            return;
        };
        let target = handle.fields();
        let [x, y, z] = target.position;
        if !self.game.level_in_place(target.level)
            || !self.game.world.loaded(x, z)
            || self.game.world.block(x, y, z) != state_id
        {
            return;
        }
        let Ok(state) = state_id.try_into_api() else {
            return;
        };
        let slot_count = self
            .game
            .world
            .block_entities
            .mod_container(state_id.0)
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
        let mut data = ResidentBlockEntityData {
            pos,
            dim: self.game.dim,
            loan,
            slot_count,
            items: self.game.res.items(),
            journal: &mut self.game.transactions,
            modified: &mut self.game.world.modified,
            chunks: &self.game.world.chunks,
            names: &self.game.world.block_names,
            hoppers: &self.game.hoppers,
            items_changed: &mut self.items_changed,
            dirty: false,
        };
        call(BlockEntityView::new(handle, state, &mut data));
    }

    fn host(&mut self) -> &mut dyn ServerHost {
        self.game
    }

    fn transaction(
        &mut self,
        call: &mut dyn FnMut(&mut dyn BlockEntityAccess, &Transaction) -> bool,
    ) {
        let target = self.target;
        let committed = mod_api::run_transaction(self.game, |game, tx| {
            let mut child = ResidentBlockEntityAccess {
                game,
                target,
                items_changed: false,
            };
            if call(&mut child, tx) {
                Ok(child.items_changed)
            } else {
                Err(())
            }
        });
        if let Ok(changed) = committed {
            self.items_changed |= changed;
        }
    }
}

struct ResidentBlockEntityData<'a> {
    pos: Pos,
    dim: Dim,
    loan: BlockEntityLoan<'a>,
    slot_count: usize,
    items: &'a Items,
    journal: &'a mut OpenTransactions,
    modified: &'a mut ModifiedChunks,
    chunks: &'a ChunkMap,
    names: &'a FxHashMap<Pos, Tag>,
    hoppers: &'a HashMap<Pos, super::super::machines::HopperState>,
    items_changed: &'a mut bool,
    dirty: bool,
}

impl ResidentBlockEntityData<'_> {
    fn entity(&self) -> &EngineBlockEntity {
        self.loan.entity
    }

    fn edit(&mut self, dirty: bool) -> &mut EngineBlockEntity {
        self.journal.journal_block_entity_loan(
            self.pos,
            &self.loan,
            self.chunks,
            self.names.get(&self.pos),
            self.hoppers.get(&self.pos).copied(),
        );
        self.dirty |= dirty;
        self.loan.entity
    }
}

impl Drop for ResidentBlockEntityData<'_> {
    fn drop(&mut self) {
        if self.dirty {
            self.journal
                .mark_block_entity_saved(self.dim, self.modified, self.pos);
        }
    }
}

impl ResidentBlockEntity for ResidentBlockEntityData<'_> {
    fn id(&self) -> &str {
        self.entity().id()
    }
    fn data_slots(&self) -> &[i32] {
        &self.entity().data
    }

    fn set_data_slot(&mut self, slot: u32, value: i32) -> bool {
        let index = slot as usize;
        if self
            .entity()
            .data
            .get(index)
            .is_none_or(|old| *old == value)
        {
            return false;
        }
        self.edit(true).data[index] = value;
        true
    }

    fn set_data_slots(&mut self, data: &[i32]) -> Result<bool, BlockEntityDataError> {
        let limit = usize::from(sunrise_mod_package::content::MAX_MENU_SLOTS);
        if data.len() > limit {
            return Err(BlockEntityDataError::TooManySlots {
                count: data.len() as u64,
                limit: limit as u64,
            });
        }
        if self.entity().data == data {
            return Ok(false);
        }
        let dest = &mut self.edit(true).data;
        dest.clear();
        dest.extend_from_slice(data);
        Ok(true)
    }

    fn nbt(&self) -> &Compound {
        &self.entity().nbt
    }

    fn set_nbt(&mut self, key: JavaString, value: Option<Tag>) -> bool {
        if [
            "id",
            "x",
            "y",
            "z",
            "Items",
            sunrise_client::containers::SHAPES_KEY,
        ]
        .iter()
        .any(|reserved| key.0.iter().copied().eq(reserved.encode_utf16()))
            || value.as_ref().is_some_and(|tag| {
                matches!(tag, Tag::End) || sunrise_client::nbt_path::too_deep(tag, 1)
            })
            || self.entity().nbt.get(&key) == value.as_ref()
        {
            return false;
        }
        let nbt = &mut self.edit(true).nbt;
        match value {
            Some(tag) => {
                nbt.insert(key, tag);
            }
            None => {
                nbt.remove(&key);
            }
        }
        true
    }

    fn item_slots(&self) -> usize {
        self.slot_count
    }

    fn item_slot(&self, slot: u32) -> Option<(&str, u32)> {
        let index = slot as usize;
        if index >= self.slot_count {
            return None;
        }
        let stack = self.entity().items.get(index)?.as_ref()?;
        Some((&stack.item, stack.count))
    }

    fn copy_item_slot(&self, slot: u32) -> Option<mod_api::ItemStack> {
        self.entity()
            .items
            .get(slot as usize)?
            .as_ref()?
            .try_into_api()
            .ok()
    }

    fn set_item_slot(
        &mut self,
        slot: u32,
        stack: Option<mod_api::ItemStack>,
    ) -> Result<bool, BlockEntityItemError> {
        let index = slot as usize;
        let slots = self.slot_count;
        if index >= slots {
            return Err(BlockEntityItemError::SlotOutOfBounds {
                slot,
                slots: slots as u32,
            });
        }
        let stack = stack
            .as_ref()
            .map(|stack| stack.try_into_engine(self.items))
            .transpose()?;
        let entity = self.entity();
        if entity.items.get(index).and_then(Option::as_ref) == stack.as_ref()
            && !entity.has_opaque_item_slot(index)
        {
            return Ok(false);
        }
        let entity = self.edit(true);
        if index >= entity.items.len() {
            entity.items.resize(slots, None);
        }
        if let Some(dest) = entity.item_slot_mut(index) {
            *dest = stack;
        }
        *self.items_changed = true;
        Ok(true)
    }

    fn set_client_update_fields(&mut self, fields: Compound) {
        if !fields.is_empty() || self.entity().has_client_update_fields() {
            self.edit(false).set_client_update_fields(fields);
        }
    }
}
