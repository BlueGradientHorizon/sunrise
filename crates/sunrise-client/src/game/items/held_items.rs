//! The player's hands and what it wears: the stack in the acting hand (`getItemInHand`), the hand of
//! the item in use and running code with the other hand acting, `Inventory.add`, and the worn glider
//! and freeze-immune gear.
use super::*;

/// `Inventory.SLOT_OFFHAND`: the `using` slot of an off-hand item.
pub(in crate::game) const OFFHAND_SLOT: usize = 40;

/// `AttackRange.effectiveMinRange`, `effectiveMaxRange` (for a player) and `hitboxMargin`.
pub(in crate::game) fn attack_reach(range: &serde_json::Value, creative: bool) -> (f32, f32, f32) {
    let value = |key: &str, default: f32| range[key].as_f64().map_or(default, |v| v as f32);
    let (min, max) = if creative {
        (
            value("min_creative_reach", 0.0),
            value("max_creative_reach", 5.0),
        )
    } else {
        (value("min_reach", 0.0), value("max_reach", 3.0))
    };
    (min, max, value("hitbox_margin", 0.3))
}

/// `AttackRange.isInRange`: float bounds, then inclusive double comparison.
pub(in crate::game) fn attack_range_contains(
    range: &serde_json::Value,
    distance: f64,
    creative: bool,
) -> bool {
    let (min, max, margin) = attack_reach(range, creative);
    in_attack_range(distance, (min, max, margin), 0.0)
}

/// `AttackRange.isInRange(attacker, box, buffer)` with inclusive eye-to-box distance.
pub(in crate::game) fn in_attack_range(
    distance: f64,
    (min, max, margin): (f32, f32, f32),
    buffer: f64,
) -> bool {
    mod_api::ItemAttackRange {
        min,
        max,
        hitbox_margin: margin,
    }
    .contains(distance, buffer)
}

impl Game<'_> {
    /// The stack in an inventory slot, the off hand at `OFFHAND_SLOT`.
    pub(in crate::game) fn stack_in(&self, slot: usize) -> Option<&ItemStack> {
        if slot == OFFHAND_SLOT {
            self.offhand.as_ref()
        } else {
            self.inventory.get(slot)?.as_ref()
        }
    }

    pub(in crate::game) fn stack_in_mut(&mut self, slot: usize) -> &mut Option<ItemStack> {
        if slot == OFFHAND_SLOT {
            &mut self.offhand
        } else {
            &mut self.inventory[slot]
        }
    }

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
            Hand::Off => OFFHAND_SLOT,
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
    pub(in crate::game) fn insert_stack_up_to(&mut self, stack: &mut ItemStack, max: u32) -> bool {
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

    /// `getAttributeValue` of an interaction range of the acting player.
    pub(in crate::game) fn interaction_range(
        &mut self,
        attribute: &str,
        survival: f64,
        creative: f64,
    ) -> f64 {
        let otherwise = if self.creative { creative } else { survival };
        self.player
            .as_mut()
            .and_then(|p| p.attribute(attribute).ok())
            .unwrap_or(otherwise)
    }

    /// `Player.entityInteractionRange`/`blockInteractionRange` for the tick's picks.
    pub(in crate::game) fn update_interaction_ranges(&mut self) {
        self.entity_reach = self.interaction_range("minecraft:entity_interaction_range", 3.0, 5.0);
        self.block_reach = self.interaction_range("minecraft:block_interaction_range", 4.5, 5.0);
    }

    /// A stack's `minecraft:attack_range` component, with spear default fallback unless removed.
    pub(in crate::game) fn attack_range_component(
        &self,
        stack: &ItemStack,
    ) -> Option<serde_json::Value> {
        let view_stack = sunrise_client::item_render::stack_view(stack);
        let view = self.pack.item_renderer.models.components_view(&view_stack);
        if let Some(val) = view.get("minecraft:attack_range") {
            return Some(val.clone());
        }
        if stack
            .data()
            .is_some_and(|d| d.other("!minecraft:attack_range").is_some())
        {
            return None;
        }
        sunrise_client::spear::Kinetic::of(&stack.item).map(|_| {
            serde_json::json!({
                "min_reach": sunrise_client::spear::REACH.min,
                "max_reach": sunrise_client::spear::REACH.max,
                "min_creative_reach": sunrise_client::spear::REACH.min_creative,
                "max_creative_reach": sunrise_client::spear::REACH.max_creative,
                "hitbox_margin": sunrise_client::spear::REACH.hitbox_margin,
            })
        })
    }

    /// `Minecraft.startAttack` on an entity: main-hand `attack_range` gate.
    pub(in crate::game) fn held_range_allows(&self, distance: f64) -> bool {
        let range = self
            .held()
            .and_then(|stack| self.attack_range_component(stack));
        range.is_none_or(|r| attack_range_contains(&r, distance, self.own_player_is_creative()))
    }

    /// `LocalPlayer.raycastHitResult`'s active item `attack_range` component.
    pub(in crate::game) fn active_attack_range(&self) -> Option<serde_json::Value> {
        let active = self
            .using
            .and_then(|(slot, _)| self.inventory.get(slot).and_then(Option::as_ref))
            .or_else(|| self.held())?;
        self.attack_range_component(active)
    }

    /// `Attributes.ATTACK_SPEED` sanitizer from the attribute catalog.
    pub(in crate::game) fn sanitize_attack_speed(&self, value: f64) -> f64 {
        self.motion
            .attributes
            .definition("minecraft:attack_speed")
            .map_or(value, |definition| definition.sanitize(value))
    }

    /// `Player.attack`: the attack damage attribute with held and effect modifiers.
    pub(in crate::game) fn attack_damage(&mut self) -> f32 {
        self.player
            .as_mut()
            .and_then(|p| p.attribute("minecraft:attack_damage").ok())
            .unwrap_or(1.0) as f32
    }

    /// `Entity.getKnownSpeed` of the mob at `index`, per second: its root vehicle's.
    pub(in crate::game) fn mob_speed(&self, index: usize) -> Vec3 {
        let mut mob = &self.mobs[index];
        while let Some(vehicle) = mob
            .extra
            .mount
            .and_then(|v| self.mobs.iter().find(|m| m.id == v))
        {
            mob = vehicle;
        }
        mob.position.subtract(mob.previous).scale(20.0)
    }

    /// The armour (head first) and off hand in `EquipmentSlot` order.
    #[allow(dead_code)]
    pub(in crate::game) fn player_equipment(&self) -> [Option<ItemStack>; 5] {
        let [head, chest, legs, feet] = self.armor.clone();
        [head, chest, legs, feet, self.offhand.clone()]
    }

    /// Shared equipment base-attribute modifier pass used by both server and client roles.
    pub(in crate::game) fn apply_equipment_modifiers_with(
        &mut self,
        applied: &mut Vec<(String, String)>,
    ) -> bool {
        if self.player.is_none() {
            return false;
        }
        let items = world_items!(self);
        let slots = ["head", "chest", "legs", "feet"];
        let mut to_add: Vec<(String, String, f64, String)> = Vec::new();
        let equipped = self
            .armor
            .iter()
            .zip(slots)
            .map(|(s, n)| (s.as_ref(), n))
            .chain([
                (self.inventory[self.selected].as_ref(), "mainhand"),
                (self.offhand.as_ref(), "offhand"),
            ]);
        for (stack, slot) in equipped {
            let Some(stack) = stack else { continue };
            for row in sunrise_client::item_attributes::attribute_modifiers(stack, items).iter() {
                if sunrise_client::living_attributes::modifier_applies(&row.slot, slot) {
                    to_add.push((
                        row.attribute.clone(),
                        row.id.clone(),
                        row.amount,
                        row.operation.clone(),
                    ));
                }
            }
        }
        let Some(player) = self.player.as_mut() else {
            return false;
        };
        for (attribute, id) in std::mem::take(applied) {
            let _ = player.set_modifier_unclamped(&attribute, &id, 0.0, "add_value", false);
        }
        for (attribute, id, amount, op) in to_add {
            if player
                .set_modifier_unclamped(&attribute, &id, amount, &op, true)
                .is_ok()
            {
                applied.push((attribute, id));
            }
        }
        true
    }

    /// `Player.resetAttackStrengthTicker`.
    pub(in crate::game) fn reset_attack_strength(&mut self) {
        self.side_reset_attack_strength();
    }

    /// `Attributes.ATTACK_SPEED` with main-hand and haste/fatigue modifiers.
    pub(in crate::game) fn main_hand_attack_speed(&self) -> f64 {
        self.side_main_hand_attack_speed()
    }

    /// `ServerPlayer.getKnownMovement` of the acting or window player.
    pub(in crate::game) fn known_player_movement(&self) -> Vec3 {
        self.side_known_player_movement()
    }

    /// Equipment attribute modifiers (`LivingEntity.collectEquipmentChanges`).
    pub(in crate::game) fn apply_equipment(&mut self) {
        self.side_apply_equipment();
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
