//! Read adapters for the evaluator's existing parameter rows.
use super::{LootDamage, LootEntity, LootLevel, LootStack};
use mod_api::{
    __private::{LootDamageAccess, LootEntityAccess, LootLevelAccess, LootStackAccess},
    LootExplorationView,
};
use serde_json::Value;
use sunrise_core::nbt::Tag;

impl super::LootToolAccess for LootStack {
    fn item_id(&self) -> &str {
        &self.item
    }

    fn enchantment_at(&self, index: usize) -> Option<(&str, i32)> {
        self.enchantments
            .get(index)
            .map(|(id, level)| (id.as_str(), *level))
    }
}

impl LootStackAccess for LootStack {
    fn count(&self) -> i32 {
        self.count
    }

    fn damage(&self) -> i32 {
        self.damage
    }

    fn map(&self) -> bool {
        self.map
    }

    fn components(&self) -> &[(String, Option<Tag>)] {
        &self.components
    }

    fn enchantments(&self) -> &[(String, i32)] {
        &self.enchantments
    }

    fn potion(&self) -> Option<&str> {
        self.potion.as_deref()
    }

    fn stew_effects(&self) -> &[(String, i32)] {
        &self.stew_effects
    }

    fn ominous_amplifier(&self) -> Option<i32> {
        self.ominous_amplifier
    }

    fn exploration(&self) -> Option<LootExplorationView<'_>> {
        let map = self.exploration.as_deref()?;
        Some(LootExplorationView::new(
            &map.destination,
            &map.decoration,
            map.zoom,
            map.search_radius,
            map.skip_existing_chunks,
        ))
    }

    fn names(&self) -> &[(String, Value)] {
        &self.names
    }
}

impl LootEntityAccess for LootEntity {
    fn kind(&self) -> &str {
        &self.kind
    }

    fn living(&self) -> bool {
        self.living
    }

    fn on_fire(&self) -> bool {
        self.on_fire
    }

    fn baby(&self) -> bool {
        self.baby
    }

    fn vehicle(&self) -> Option<&dyn LootEntityAccess> {
        self.vehicle
            .as_deref()
            .map(|entity| entity as &dyn LootEntityAccess)
    }

    fn cube_size(&self) -> Option<i32> {
        self.cube_size
    }

    fn sheep(&self) -> Option<(&str, bool)> {
        self.sheep
            .as_ref()
            .map(|(color, sheared)| (color.as_str(), *sheared))
    }

    fn captain(&self) -> Option<bool> {
        self.captain
    }

    fn frog_variant(&self) -> Option<&str> {
        self.frog_variant.as_deref()
    }

    fn chicken_variant(&self) -> Option<&str> {
        self.chicken_variant.as_deref()
    }

    fn powered(&self) -> bool {
        self.powered
    }

    fn equipment(&self, slot: &str) -> Option<&dyn LootStackAccess> {
        self.equipment
            .iter()
            .find(|(name, _)| name == slot)
            .map(|(_, stack)| stack as &dyn LootStackAccess)
    }

    fn equipment_at(&self, index: usize) -> Option<(&str, &dyn LootStackAccess)> {
        self.equipment
            .get(index)
            .map(|(slot, stack)| (slot.as_str(), stack as &dyn LootStackAccess))
    }
}

impl LootDamageAccess for LootDamage {
    fn kind(&self) -> &str {
        &self.kind
    }

    fn source(&self) -> Option<&dyn LootEntityAccess> {
        self.source
            .as_ref()
            .map(|entity| entity as &dyn LootEntityAccess)
    }

    fn direct(&self) -> Option<&dyn LootEntityAccess> {
        self.direct
            .as_ref()
            .map(|entity| entity as &dyn LootEntityAccess)
    }
}

impl LootLevelAccess for LootLevel {
    fn dimension(&self) -> &str {
        &self.dimension
    }

    fn game_time(&self) -> i64 {
        self.game_time
    }

    fn day_time(&self) -> i64 {
        self.day_time
    }

    fn raining(&self) -> bool {
        self.raining
    }

    fn thundering(&self) -> bool {
        self.thundering
    }
}
