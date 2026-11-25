//! Item/enchantment answers resolved when parts install, read from registry snapshots.
#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// Preserve installed answers when publishing a fresh world-loot snapshot,
    /// including loads after parts install. This never asks a listener twice.
    pub(in crate::game) fn copy_loot_enchantment_answers(
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
