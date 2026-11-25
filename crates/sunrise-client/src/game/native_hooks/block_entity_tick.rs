#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::{
    Dim,
    api_values::{TryIntoApi, TryIntoEngine},
    transactions::OpenTransactions,
};
#[allow(unused_imports)]
use mod_api::{
    BlockEntityAccess, BlockEntityItemError, BlockEntityRef, BlockEntityScope, BlockEntityView,
    ResidentBlockEntity, ServerHost, TransactionContext, TransactionScopeError,
    TransactionUnavailable,
};
#[allow(unused_imports)]
use std::collections::HashMap;
#[allow(unused_imports)]
use sunrise_client::{
    block_entities::BlockEntityLoan, chunk_map::ChunkMap, fast_hash::FxHashMap,
    world::ModifiedChunks,
};
#[allow(unused_imports)]
use sunrise_core::nbt::{Compound, JavaString, Tag};
#[allow(unused_imports)]
use sunrise_items::items::Items;

/// Dispatch the one resident server ticker; the scope owns access to the Host.
pub(in crate::game) fn native_block_entity_tick(
    kind: mod_api::BlockEntityTypeKey,
    subject: mod_api::BlockEntityScope<'_>,
) {
    crate::game::native_hooks::fire::fire_block_entity_tick(kind, subject);
}

pub(in crate::game) fn valid_tick_subject(block: &str, entity: &EngineBlockEntity) -> bool {
    sunrise_client::block_entity_types::valid(block, entity.id()) == Some(true)
}

#[derive(Clone, Copy)]
pub(in crate::game) enum Loan {
    // Reuse the ticker's validity result for this instance and state.
    Tick(Option<BlockStateId>),
    Menu(BlockStateId),
    OnLoad,
}

pub(in crate::game) struct ResidentBlockEntityAccess<'g, 'w> {
    pub(in crate::game) game: &'g mut Game<'w, crate::game::server_side::ServerState>,
    pub(in crate::game) subject: Option<BlockEntityRef>,
    pub(in crate::game) loan: Loan,
    pub(in crate::game) items_changed: bool,
}

impl<'g, 'w> ResidentBlockEntityAccess<'g, 'w> {
    pub(in crate::game) fn new(
        game: &'g mut Game<'w, crate::game::server_side::ServerState>,
        subject: Option<BlockEntityRef>,
        loan: Loan,
    ) -> Self {
        Self {
            game,
            subject,
            loan,
            items_changed: false,
        }
    }

    pub(in crate::game) fn on_load(game: &'g mut Game<'w, crate::game::server_side::ServerState>, subject: BlockEntityRef) -> Self {
        Self::new(game, Some(subject), Loan::OnLoad)
    }
}

pub(in crate::game) struct ServerBlockEntityData<'a> {
    pub(in crate::game) dim: Dim,
    pub(in crate::game) pos: Pos,
    pub(in crate::game) loan: BlockEntityLoan<'a>,
    pub(in crate::game) slot_count: usize,
    pub(in crate::game) items: &'a Items,
    pub(in crate::game) journal: &'a mut OpenTransactions,
    pub(in crate::game) chunks: &'a ChunkMap,
    pub(in crate::game) names: &'a FxHashMap<Pos, Tag>,
    pub(in crate::game) hoppers: &'a HashMap<Pos, super::super::machines::HopperState>,
    pub(in crate::game) menu: Option<(&'a mut ModifiedChunks, &'a mut bool)>,
    pub(in crate::game) dirty: bool,
}

impl ServerBlockEntityData<'_> {
    pub(in crate::game) fn entity(&self) -> &EngineBlockEntity {
        self.loan.entity
    }

    pub(in crate::game) fn edit(&mut self, saved: bool) -> &mut EngineBlockEntity {
        self.dirty |= saved;
        self.journal.journal_block_entity_loan(
            self.dim,
            self.pos,
            &self.loan,
            self.chunks,
            self.names.get(&self.pos),
            self.hoppers.get(&self.pos).copied(),
        );
        self.loan.entity
    }
}

impl Drop for ServerBlockEntityData<'_> {
    fn drop(&mut self) {
        if self.dirty
            && let Some((modified, _)) = &mut self.menu
        {
            let (x, _, z) = self.pos;
            self.journal
                .mark_scoped_menu_write(self.dim, (x >> 4, z >> 4), modified);
        }
    }
}

impl ResidentBlockEntity for ServerBlockEntityData<'_> {
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
        self.edit(false).data[index] = value;
        true
    }

    fn set_data_slots(&mut self, data: &[i32]) -> bool {
        if self.entity().data == data {
            return false;
        }
        let dest = &mut self.edit(false).data;
        dest.clear();
        dest.extend_from_slice(data);
        true
    }

    fn nbt(&self) -> &Compound {
        &self.entity().nbt
    }

    fn set_nbt(&mut self, key: JavaString, value: Option<Tag>) -> bool {
        let is_items = key.0.iter().copied().eq("Items".encode_utf16());
        if ["id", "x", "y", "z", sunrise_client::containers::SHAPES_KEY]
            .iter()
            .any(|reserved| key.0.iter().copied().eq(reserved.encode_utf16()))
            || (is_items && self.slot_count > 0)
            || value.as_ref().is_some_and(|tag| {
                matches!(tag, Tag::End) || sunrise_client::nbt_path::too_deep(tag, 1)
            })
        {
            return false;
        }
        let unchanged = if is_items && !self.entity().items.is_empty() {
            sunrise_client::save::block_entity_encode::mod_items(self.entity(), self.items) == value
        } else {
            self.entity().nbt.get(&key) == value.as_ref()
        };
        if unchanged {
            return false;
        }
        let entity = self.edit(true);
        if is_items {
            entity.items.clear();
        }
        match value {
            Some(tag) => {
                entity.nbt.insert(key, tag);
            }
            None => {
                entity.nbt.remove(&key);
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
        if let Some((_, changed)) = &mut self.menu {
            **changed = true;
        }
        let entity = self.edit(true);
        if index >= entity.items.len() {
            entity.items.resize(slots, None);
        }
        if let Some(dest) = entity.item_slot_mut(index) {
            *dest = stack;
        }
        Ok(true)
    }

    fn set_client_update_fields(&mut self, fields: Compound) {
        if !fields.is_empty() || self.entity().has_client_update_fields() {
            self.edit(false).set_client_update_fields(fields);
        }
    }
}
