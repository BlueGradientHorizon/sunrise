use super::block_entity_client_tick::ClientBlockEntityAccess;
use super::*;
use crate::game::{
    Dim,
    api_values::{TryIntoApi, TryIntoEngine},
    transactions::OpenTransactions,
};
use mod_api::{
    BlockEntityClientScope, BlockEntityDataError, BlockEntityItemError, BlockEntityRef,
    BlockEntitySideScope, BlockEntityTickAccess, BlockEntityTickScope, BlockEntityView,
    ResidentBlockEntity, ServerHost,
};
use std::collections::HashMap;
use sunrise_client::{
    block_entities::BlockEntityLoan, chunk_map::ChunkMap, fast_hash::FxHashMap,
    world::ModifiedChunks,
};
use sunrise_core::nbt::{Compound, JavaString, Tag};
use sunrise_items::items::Items;

struct OnLoadServerAccess<'g, 'w> {
    game: &'g mut Game<'w>,
    subject: BlockEntityRef,
}

impl BlockEntityTickAccess for OnLoadServerAccess<'_, '_> {
    fn with_view(&mut self, call: &mut dyn FnMut(BlockEntityView<'_>)) {
        use mod_api::__private::game::Fields;
        let target = self.subject.fields();
        let [x, y, z] = target.position;
        if !self.game.level_in_place(target.level) || !self.game.world.loaded(x, z) {
            return;
        }
        let state_id = self.game.world.block(x, y, z);
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
        let mut data = OnLoadServerData {
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
            dirty: false,
        };
        call(BlockEntityView::new(self.subject, state, &mut data));
    }

    fn host(&mut self) -> &mut dyn ServerHost {
        self.game
    }
}

struct OnLoadServerData<'a> {
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
    dirty: bool,
}

impl OnLoadServerData<'_> {
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

impl Drop for OnLoadServerData<'_> {
    fn drop(&mut self) {
        if self.dirty {
            self.journal
                .mark_block_entity_saved(self.dim, self.modified, self.pos);
        }
    }
}

impl ResidentBlockEntity for OnLoadServerData<'_> {
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
        Ok(true)
    }

    fn set_client_update_fields(&mut self, fields: Compound) {
        if !fields.is_empty() || self.entity().has_client_update_fields() {
            self.edit(false).set_client_update_fields(fields);
        }
    }
}

impl Game<'_> {
    /// Configure notice collection before this level can receive any block entities.
    pub(in crate::game) fn world_for_mods(
        air: BlockStateId,
        seed: i64,
    ) -> sunrise_client::world::World {
        let mut world = sunrise_client::world::World::new(air, seed);
        world
            .block_entities
            .set_on_load_enabled(mod_api::__private::has_part_hook(
                mod_api::HookId::BlockEntityOnLoad,
            ));
        world
            .block_entities
            .set_data_load_enabled(mod_api::__private::has_part_hook(
                mod_api::HookId::BlockEntityLoadAdditional,
            ));
        world
    }

    /// A replacement can inherit a ticker slot later in the current pass.
    /// Deliver its attachment notice before lending its resident row to that tick.
    pub(in crate::game) fn tick_mod_block_entity(&mut self, pos: Pos) {
        if mod_api::__private::any_mod_disabled()
            && let Some(BlockEntity::Mod(entity)) = self.world.block_entities.get(&pos)
            && mod_api::__private::content_owner_disabled(entity.id())
        {
            return;
        }
        if self.world.block_entities.has_pending_on_load()
            || self.world.block_entities.has_pending_data_load()
        {
            let generation = self.world.block_entities.generation(&pos);
            self.fire_pending_block_entity_loads();
            if self.world.block_entities.generation(&pos) != generation {
                return;
            }
        }
        self.tick_mod_block_entity_loaded(pos);
    }

    /// Drain attachment notices before either logical side runs block entity tickers.
    pub(in crate::game) fn fire_pending_block_entity_loads(&mut self) {
        self.fire_pending_block_entity_data_loads();
        if !self.world.block_entities.has_pending_on_load() {
            return;
        }
        let server = self.is_logical_server();
        let level = if server {
            self.level(self.dim.id())
        } else {
            self.native_client_level()
        };
        let Some(level) = level else { return };
        let mut remaining = self.world.block_entities.pending_on_load_len();
        while let Some((pos, generation)) = self
            .world
            .block_entities
            .next_on_load_in_batch(&mut remaining)
        {
            if !mod_api::__private::has_part_hook(mod_api::HookId::BlockEntityOnLoad) {
                continue;
            }
            let Some(BlockEntity::Mod(entity)) = self.world.block_entities.get(&pos) else {
                continue;
            };
            let Some(id) = mod_api::__private::block_entity_type_key(entity.id()) else {
                continue;
            };
            let subject = mod_api::__private::game::block_entity_ref(
                level,
                [pos.0, pos.1, pos.2],
                generation,
            );
            if server {
                let mut access = OnLoadServerAccess {
                    game: self,
                    subject,
                };
                Self::native_block_entity_on_load(mod_api::__private::BlockEntityOnLoadCall {
                    id,
                    subject: BlockEntitySideScope::Server(BlockEntityTickScope::new(&mut access)),
                });
            } else {
                let mut access = ClientBlockEntityAccess {
                    game: self,
                    subject,
                };
                Self::native_block_entity_on_load(mod_api::__private::BlockEntityOnLoadCall {
                    id,
                    subject: BlockEntitySideScope::Client(BlockEntityClientScope::new(&mut access)),
                });
            }
        }
    }

    /// Notify subscribers after this instance is attached and its saved data is applied.
    pub(in crate::game) fn native_block_entity_on_load(
        call: mod_api::__private::BlockEntityOnLoadCall<'_>,
    ) {
        Self::fire_block_entity_on_load(call);
    }
}
