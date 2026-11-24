#[allow(unused_imports)]
pub(in crate::game::server_side) use super::super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::tables::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{collections::HashMap, sync::Arc};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::items::EntityTypeTags;

impl Game<'_> {
    /// Apply the server's entity type tags on every client, including the
    /// integrated server's window. Preserve the integrated client's block
    /// and item tables.
    pub(in crate::game) fn install_server_items(&mut self) {
        let Some(remote) = self.remote.as_ref() else {
            return;
        };
        let names = |registry: &str| -> Option<HashMap<String, Vec<String>>> {
            let ids = remote.builtin.get(registry)?;
            let tags = remote
                .registries
                .tags
                .get(registry)?
                .iter()
                .map(|(name, members)| {
                    let members = members.iter().filter_map(|&id| ids.name(id));
                    (name.clone(), members.map(str::to_owned).collect())
                });
            Some(tags.collect())
        };
        let (blocks, items) = if self.integrated.is_none() {
            (names("minecraft:block"), names("minecraft:item"))
        } else {
            (None, None)
        };
        let base = self
            .world_tables
            .0
            .as_ref()
            .map_or_else(|| self.res.items(), |tables| &tables.items);
        let mut items = base.with_tags(blocks, items);
        let tags = EntityTypeTags::from_ordered(
            names(EntityTypeTags::REGISTRY).unwrap_or_default(),
            base.entity_type_tags.ids().clone(),
        );
        items.set_entity_type_tags(tags);
        if let Some(enchantments) = self.remote.as_mut().and_then(|r| r.enchantments.as_mut()) {
            Arc::make_mut(enchantments).entity_tags = items.entity_type_tags.members().clone();
        }
        self.server_items = Some(Arc::new(items));
    }
}
