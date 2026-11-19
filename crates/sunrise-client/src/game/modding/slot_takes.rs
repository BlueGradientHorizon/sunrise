//! `menu_slot_take` on the game's side: each take a click made from a mod menu type's own slots
//! (`Menu::slot_takes`, noted at `Slot.onTake`), heard by the parts of the mod block entity the
//! menu is open on, with the block entity out of the world. The takes and the listeners are one
//! action: a listener that fails has its host writes undone, and the caller undoes the take.
use super::Game;
use super::api_values::{TryIntoApi, finish_block_entity_writeback};
use sunrise_client::{
    block_entities::Pos,
    containers::{BlockEntity, ModBlockEntity},
    items::ItemStack,
};

/// A take noted at `Slot.onTake`: the menu slot, the stack taken and what the slot holds after.
pub(in crate::game) type SlotTake = (usize, ItemStack, Option<ItemStack>);

impl Game<'_> {
    /// `menu_slot_take` for `takes`, which a click by `player` on mod menu type `menu` made from
    /// the mod block entity at `pos`. False when a listener failed: its host writes are undone,
    /// the block entity is as the click left it, and the caller undoes the take.
    pub(super) fn mod_menu_slot_takes(
        &mut self,
        pos: Pos,
        player: u128,
        menu: &str,
        takes: &[SlotTake],
    ) -> bool {
        let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&pos) else {
            return true;
        };
        if m.id().is_empty() {
            return true;
        }
        let mut entity = std::mem::take(&mut **m);
        let items = entity.items.clone();
        let heard = self.api_menu_slot_takes(&mut entity, player, menu, takes);
        self.put_back_mod_block_entity(pos, entity, &items);
        heard
    }

    /// `menu_slot_take` for `takes` from `entity`, already out of the world: the parts' writes
    /// to it come back only when every listener returned. False when one failed (panicked or
    /// trapped), its host writes undone by the transaction they ran in. A take that cannot cross
    /// to the API is not heard.
    pub(super) fn api_menu_slot_takes(
        &mut self,
        entity: &mut ModBlockEntity,
        player: u128,
        menu: &str,
        takes: &[SlotTake],
    ) -> bool {
        if !mod_api::__private::has_part_hook(mod_api::HookId::MenuSlotTake) {
            return true;
        }
        let Ok(mut api) = entity.try_into_api() else {
            return true;
        };
        let revision = api.revision();
        let takes: Vec<_> = takes
            .iter()
            .filter_map(|(slot, taken, left)| {
                Some((*slot, taken.try_into_api().ok()?, left.try_into_api().ok()?))
            })
            .collect();
        let heard = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            mod_api::run_transaction(self, |game, _| {
                for (slot, taken, left) in &takes {
                    game.native_menu_slot_take(&mut api, player, menu, *slot, taken, left.as_ref());
                }
                Ok::<_, ()>(())
            })
        }));
        if heard.is_err() {
            log::error!("menu_slot_take: a listener failed for {menu}; the take is undone");
            return false;
        }
        let items = self.items();
        let _ =
            finish_block_entity_writeback(entity, &api, revision, items, "menu_slot_take", Ok(()));
        true
    }
}
