#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_recipe_properties::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

impl Game<'_> {
    /// Publish once per owning load or server update, never from a slot read.
    pub(in crate::game) fn refresh_recipe_property_context(&mut self) {
        let mut sets = HashMap::<String, HashSet<String>>::new();
        if let Some(server) = &self.recipe_ui.0.server_properties.sets {
            if let Some(remote) = &self.remote {
                for (key, ids) in server
                    .iter()
                    .filter(|(key, _)| !key.starts_with("minecraft:"))
                {
                    let items = ids
                        .iter()
                        .filter_map(|id| remote.builtin.get("minecraft:item")?.name(*id))
                        .filter(|name| *name != "minecraft:air")
                        .map(str::to_owned)
                        .collect();
                    sets.insert(key.clone(), items);
                }
            }
        } else if self.has_world_tables() || self.res.rest.get().is_some() {
            for (key, names) in self.recipes().custom_property_sets() {
                let items = names
                    .iter()
                    .filter(|name| self.res.items().get(name).is_some())
                    .cloned()
                    .collect();
                sets.insert(key.to_owned(), items);
            }
        }
        let tags = self
            .native_tag_items()
            .filter(|items| items.tags_loaded(sunrise_items::items::TagRegistry::Item))
            .and_then(|items| {
                // Only members in this generation's item registry can be exposed.
                let registry = self
                    .items()
                    .registry_order
                    .iter()
                    .map(AsRef::as_ref)
                    .collect();
                mod_api::__private::BoundItemTags::bind(&items.item_tag_order, &registry)
            });
        let context = Arc::make_mut(&mut self.loop_context);
        context.set_recipe_properties(Arc::new(sets));
        context.set_item_tags(tags);
        if let Some(host) = self.lan.as_mut() {
            host.hub.loop_context = self.loop_context.clone();
        }
    }
}
