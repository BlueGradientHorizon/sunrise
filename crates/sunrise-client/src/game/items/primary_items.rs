//! Item/enchantment answers resolved when parts install, read from registry snapshots.
use super::*;

impl Game<'_> {
    /// Server half of `resolve_primary_items`: install support and `IsPrimaryItemFor` answers.
    pub(in crate::game) fn server_resolve_primary_items_server(
        &self,
        support: sunrise_world::enchantment_answers::Answers,
    ) {
        if let Some(loot) = self.server_loot.0.get() {
            loot.support_answers().install(support);
        }
        let enchantments = self.item_enchantments();
        let items = self.items();
        let mut answers = sunrise_world::enchantment_answers::Answers::new();
        if mod_api::__private::has_part_hook(mod_api::HookId::IsPrimaryItemFor) {
            for item in items.items.keys() {
                if !mod_api::__private::item_listens(mod_api::HookId::IsPrimaryItemFor, item) {
                    continue;
                }
                let stack = ItemStack::new(item, 1);
                for enchantment in &enchantments.list {
                    let vanilla = enchantments.is_primary(items, enchantment, item);
                    let mut primary = vanilla;
                    self.native_is_primary_item_for(&stack, &enchantment.id, &mut primary);
                    if primary != vanilla {
                        answers
                            .entry(item.clone())
                            .or_default()
                            .insert(enchantment.id.clone(), primary);
                    }
                }
            }
        }
        for table in self.enchantment_snapshots() {
            table.primary_answers().install(answers.clone());
        }
        if let Some(loot) = self.server_loot.0.get() {
            loot.primary_answers().install(answers);
        }
    }

    #[inline(always)]
    pub(in crate::game) fn server_clear_loot_enchantment_answers(&self) {
        if let Some(loot) = self.server_loot.0.get() {
            loot.primary_answers().clear();
            loot.support_answers().clear();
        }
    }

    /// Preserve installed answers when publishing a fresh world-loot snapshot,
    /// including loads after parts install. This never asks a listener twice.
    pub(in crate::game::server_side) fn copy_loot_enchantment_answers(
        &self,
        loot: &sunrise_world::loot::LootData,
    ) {
        let enchantments = &self.stations().enchantments;
        loot.primary_answers()
            .install(enchantments.primary_answers().snapshot());
        loot.support_answers()
            .install(enchantments.support_answers().snapshot());
    }
}
