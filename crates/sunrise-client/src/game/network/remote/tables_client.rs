#[allow(unused_imports)]
pub(in crate::game::client_side) use super::super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tables::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{collections::HashMap, sync::Arc};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::EntityTypeTags;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Apply synced block, item, fluid, and entity-type tags plus generated class tags on clients.
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
        let (blocks, items) = (names("minecraft:block"), names("minecraft:item"));
        let base = self.res.items();
        let mut items = base.with_tags(blocks, items);
        if let Some(fluids) = names("minecraft:fluid") {
            items.set_tags(sunrise_items::items::TagRegistry::Fluid, fluids);
        }
        crate::game::class_tags::install(&mut items, self.res.facts());
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
