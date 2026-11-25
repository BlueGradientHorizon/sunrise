//! The player's hands and what it wears: the stack in the acting hand (`getItemInHand`), the hand of
//! the item in use and running code with the other hand acting, `Inventory.add`, and the worn glider
//! and freeze-immune gear.
use super::*;

impl Game<'_> {
    /// The stack in the hand that acts (`getItemInHand(hand)`).
    pub(in crate::game) fn held(&self) -> Option<&ItemStack> {
        self.stack_in(self.held_slot())
    }

    pub(in crate::game) fn held_mut(&mut self) -> &mut Option<ItemStack> {
        self.stack_in_mut(self.held_slot())
    }

    /// The slot of the hand that acts: the selected one, or `OFFHAND_SLOT`.
    /// `LivingEntity.canGlide`: some worn piece, or a stack in a hand, glides
    /// there (`canGlideUsing` walks every `EquipmentSlot`: `gliding_slots`).
    pub(in crate::game) fn glider_worn(&self) -> bool {
        super::glide_wear::gliding_slots(
            self.res.items(),
            &self.armor,
            self.inventory[self.selected].as_ref(),
            self.offhand.as_ref(),
        )
        .next()
        .is_some()
    }

    /// `LivingEntity.updateFallFlying`: the glide's wear. `Util.getRandom`
    /// picks one of the gliders that will not break (from the entity's own
    /// random), and
    /// `ItemStack.hurtAndBreak` takes `points` of its durability (Unbreaking
    /// may save it). Callers skip creative.
    pub(in crate::game) fn wear_glider(&mut self, points: u32) {
        self.side_wear_glider(points);
    }

    /// `LivingEntity.canFreeze`'s armor half: any worn piece in
    /// `freeze_immune_wearables`.
    pub(crate) fn wears_freeze_immune(&self) -> bool {
        let immune = self
            .res
            .items()
            .item_tags
            .get("minecraft:freeze_immune_wearables");
        self.armor
            .iter()
            .flatten()
            .any(|s| immune.is_some_and(|t| t.contains(&*s.item)))
    }

    pub(in crate::game) fn held_slot(&self) -> usize {
        match self.hand {
            Hand::Main => self.selected,
            Hand::Off => shield::OFFHAND_SLOT,
        }
    }

    /// `LivingEntity.getUsedItemHand`: the hand of the item in use.
    pub(in crate::game) fn used_hand(&self) -> Option<Hand> {
        item_use::hand_of(self.using)
    }

    /// Run `f` with `hand` acting (`held`, `consume_held`, `swing_arm`).
    pub(in crate::game) fn with_hand<T>(
        &mut self,
        hand: Hand,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let prior = std::mem::replace(&mut self.hand, hand);
        let out = f(self);
        self.hand = prior;
        out
    }

    /// Inventory.add: returns the stack that did not fit.
    pub(in crate::game) fn add_stack_to_inventory(
        &mut self,
        mut stack: ItemStack,
    ) -> Option<ItemStack> {
        self.insert_stack_into_inventory(&mut stack)
            .then_some(stack)
    }

    /// Consumes the fitted count, leaving the offered item and components in `stack`.
    /// Returns true if some of the count remains.
    pub(in crate::game) fn insert_stack_into_inventory(&mut self, stack: &mut ItemStack) -> bool {
        let max = self
            .res
            .items()
            .get(&stack.item)
            .map_or(64, |i| i.max_stack);
        self.insert_stack_up_to(stack, max)
    }

    /// `insert_stack_into_inventory` with `max` the stack's own maximum.
    pub(in crate::game) fn insert_stack_up_to(
        &mut self,
        stack: &mut ItemStack,
        max: u32,
    ) -> bool {
        let before = stack.count;
        let left = inventory_add(
            &mut self.inventory,
            &mut self.offhand,
            self.selected,
            max,
            stack,
        );
        if stack.count < before {
            self.side_on_inventory_item_added(stack);
        }
        left
    }
}

/// The item of the stack in `selected` (`Player.tick`'s main-hand item), borrowed: a guest's
/// tick compares it with the last one and clones it only when it changed.
pub(in crate::game) fn held_attack_item(
    inventory: &[Option<ItemStack>],
    selected: usize,
) -> Option<&Arc<str>> {
    inventory.get(selected)?.as_ref().map(|s| &s.item)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item: &str) -> ItemStack {
        ItemStack {
            item: item.into(),
            count: 1,
            damage: 0,
            extra: None,
        }
    }

    #[test]
    fn held_attack_item_matches_the_cloned_stack() {
        let mut inventory: Vec<Option<ItemStack>> = vec![None; 36];
        inventory[0] = Some(stack("minecraft:stone"));
        inventory[3] = Some(stack("minecraft:diamond_sword"));
        for selected in [0, 1, 3, 35, 36, 1000] {
            // The clone the tick used to make, as the reference.
            let cloned = inventory.get(selected).cloned().flatten().map(|s| s.item);
            assert_eq!(held_attack_item(&inventory, selected).cloned(), cloned);
        }
        assert_eq!(held_attack_item(&[], 0), None);
    }
}
