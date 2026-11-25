#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::api_values::{
    TryIntoApi, TryIntoEngine, stack_value,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    CreativeModeTabs, CreativeTabEditError, CreativeTabList, CreativeTabPermissions,
    CreativeTabReadError, CreativeTabVisibility,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_items::creative_catalog::{
    Catalog, Holders, Parameters,
};

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn client_creative_holders(&self) -> Option<Holders> {
        self.remote.as_ref().map(|remote| {
            sunrise_client::creative::live_holders(&remote.builtin, &remote.registries)
        })
    }

    pub(in crate::game) fn client_creative_catalog(&self) -> &Arc<Catalog> {
        self.side.creative_screen.0.catalog()
    }

    pub(in crate::game) fn client_reset_creative_screen(&mut self) {
        self.side.creative_screen.0.reset_contents();
    }

    /// The name components that replace the creative search's prepared name lines.
    pub(in crate::game::client_side) fn refresh_creative_search_names(&mut self) {
        // Only when the search tab's stacks were made again, not every creative tick.
        let generation = self.side.creative_screen.0.search_generation();
        if self.side.creative_search_named == Some(generation) {
            return;
        }
        self.side.creative_search_named = Some(generation);
        let names = self
            .side
            .creative_screen
            .0
            .search_items()
            .iter()
            .map(|stack| {
                if sunrise_client::stations::names_itself(stack) {
                    Some(self.stack_hover_name_text(stack))
                } else {
                    self.mod_item_name(stack)
                }
            })
            .collect();
        self.side.creative_screen.0.set_search_names(names);
    }

    /// The screen uses the completed cache, with the same holder definitions for search text.
    pub(in crate::game::client_side) fn refresh_creative_tabs(&mut self) {
        let Ok(lists) = self.build_creative(self.creative_permissions()) else {
            return;
        };
        if let Some(published) = &self.creative_contents.published
            && Arc::ptr_eq(&lists, &published.lists)
        {
            self.side.creative_screen.0.publish(
                lists,
                published.holders.clone(),
                published.items.clone(),
                &self.pack.names,
            );
            self.refresh_creative_search_names();
        }
    }

    /// The built-in sorter is only a reader. A cold sort previews stock order
    /// without publishing anything or giving mods a creative callback.
    pub(in crate::game::client_side) fn stock_sort_order(&self) -> Vec<ItemStack> {
        let preview;
        let lists = if let Some(published) = &self.creative_contents.published {
            &published.lists
        } else {
            preview = self.side.creative_screen.0.catalog().build(
                Parameters {
                    features: self.enabled_feature_ids(),
                    operator: false,
                    holders: &self.client_creative_holders().unwrap_or_default(),
                    items: self.items(),
                },
                None,
                |_, _, _| {},
            );
            &preview
        };
        self.side.creative_screen.0.search_order_from(lists)
    }
}

#[cfg(test)]
#[path = "build_creative_mode_tab_contents/tests_client.rs"]
pub(in crate::game) mod tests;
