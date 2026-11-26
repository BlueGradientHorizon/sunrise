//! Server-side item entity transaction journal writers.
use crate::game::ServerGame;
use sunrise_gameplay_common::game::transactions::{After, KeptItemEntity};
use sunrise_items::items::ItemStack;

impl ServerGame<'_> {
    /// Sets the stack of `id` for an item place. A caller that already has a
    /// validated resident slot uses `journal_set_item_entity_at` instead.
    pub(in crate::game) fn journal_set_item_entity(&mut self, id: u32, stack: ItemStack) -> bool {
        self.common
            .dropped
            .iter()
            .position(|item| item.id == id)
            .is_some_and(|index| self.journal_set_item_entity_at(index, stack))
    }

    /// An item emptied in a transaction stays, empty, until root commit removes it.
    pub(in crate::game) fn journal_set_item_entity_at(
        &mut self,
        index: usize,
        stack: ItemStack,
    ) -> bool {
        let Some(level) = self.common.transactions.level_frame(self.common.dim) else {
            return self.native_set_item_entity(index, stack);
        };
        let item = &mut self.common.dropped[index];
        level
            .item_entities
            .entry(item.join)
            .or_default()
            .stack
            .get_or_insert_with(|| item.stack.clone());
        if stack.count == 0 {
            level.after.push(After::Emptied(item.id));
        }
        item.stack = stack;
        true
    }

    /// Lend an item in the active level and the first-write journal of its innermost open frame.
    pub(in crate::game) fn journal_item_fields(
        &mut self,
        index: usize,
    ) -> (
        &mut sunrise_sim::entities::ItemEntity,
        Option<&mut KeptItemEntity>,
    ) {
        let item = &mut self.common.dropped[index];
        let kept = self
            .common
            .transactions
            .level_frame(self.common.dim)
            .map(|level| level.item_entities.entry(item.join).or_default());
        (item, kept)
    }

    pub(in crate::game) fn journal_set_item_pickup_delay(&mut self, index: usize, ticks: i32) {
        let (item, kept) = self.journal_item_fields(index);
        if let Some(kept) = kept {
            kept.pickup_delay.get_or_insert(item.pickup_delay);
        }
        item.pickup_delay = i64::from(ticks);
    }

    pub(in crate::game) fn journal_set_item_target(
        &mut self,
        index: usize,
        target: Option<[i32; 4]>,
    ) {
        let (item, kept) = self.journal_item_fields(index);
        if let Some(kept) = kept {
            kept.target.get_or_insert(item.target);
        }
        item.target = target;
    }
}
