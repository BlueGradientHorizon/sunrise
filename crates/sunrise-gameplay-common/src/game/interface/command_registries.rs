//! The command registries the game keeps in step with what the world's content and data
//! packs define, outside the one-time build in `chat.rs`.
use crate::game::CommonGame;

impl CommonGame<'_> {
    /// Active enchantment registry snapshot for command codecs and completions.
    /// Replace both ID views; keep the existing registry tags intact.
    pub fn sync_enchantment_registries(&mut self) {
        let mut ids: Vec<String> = self
            .active_enchantment_definitions()
            .list
            .iter()
            .map(|entry| entry.id.to_string())
            .collect();
        ids.sort();
        ids.dedup();
        self.ui.registries.enchantments = ids.clone();
        self.ui
            .registries
            .by_registry
            .entry("minecraft:enchantment".into())
            .or_default()
            .ids = ids;
    }

    /// `ResourceOrIdArgument`'s loot tables from the active item tables.
    pub fn sync_loot_table_ids(&mut self) {
        let mut ids: Vec<String> = self.items().loot_table_ids().map(str::to_owned).collect();
        ids.sort();
        if let Some(entry) = self
            .ui
            .registries
            .by_registry
            .get_mut("minecraft:loot_table")
        {
            entry.ids.clone_from(&ids);
        }
        self.ui.registries.loot_tables = ids;
    }

    /// Item and block tags bound into command registries from the active item tables.
    pub fn sync_pack_tag_ids(&mut self) {
        let tags = |ids: &std::collections::HashMap<String, std::collections::HashSet<String>>| {
            let mut out: Vec<String> = ids
                .keys()
                .map(|k| sunrise_command::args::normalize(k))
                .collect();
            out.sort();
            out
        };
        let item_tags = tags(&self.items().item_tags);
        let block_tags = tags(&self.items().block_tags);
        let reg = &mut self.ui.registries;
        reg.item_tags.clone_from(&item_tags);
        reg.block_tags.clone_from(&block_tags);
        // Only update exported registry entries so a new empty entry does not hide item ids.
        for (registry, tags) in [
            ("minecraft:item", item_tags),
            ("minecraft:block", block_tags),
        ] {
            if let Some(entry) = reg.by_registry.get_mut(registry) {
                entry.tags = tags;
            }
        }
    }
}

use crate::game::tables::GameItems as _;

use crate::game::tables::WorldTablesExt as _;
