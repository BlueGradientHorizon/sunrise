//! `LivingEntity.isBlocking` for native mods through a `LivingRef`: a player or a mob holding up
//! an item that blocks attacks, past the item's block delay.
use super::{Game, held_items::block_delay_passed};
use mod_api::{EquipmentSlot, LivingRef, client::first_person::Hand};

impl crate::game::CommonGame<'_> {
    /// `LivingEntity.isBlocking` (`getItemBlockingWith` is not null): the entity uses an item
    /// that blocks attacks, from either hand, and has used it for the component's block delay.
    /// False while it uses nothing or another item. None for armor stands, the dragon and a
    /// stale handle.
    pub(in crate::game) fn living_is_blocking(&self, living: LivingRef) -> Option<bool> {
        let ticks = self.side_living_use_ticks(living)?;
        let blocking = ticks.is_some_and(|ticks| {
            self.side_living_used_item_hand(living)
                .and_then(|hand| {
                    let slot = match hand {
                        Hand::Main => EquipmentSlot::MainHand,
                        Hand::Off => EquipmentSlot::OffHand,
                    };
                    self.living_equipment_stack(living, slot)
                })
                .is_some_and(|stack| {
                    self.blocks_attacks(&stack.project())
                        .is_some_and(|component| block_delay_passed(&component, ticks))
                })
        });
        Some(blocking)
    }
}

use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;
