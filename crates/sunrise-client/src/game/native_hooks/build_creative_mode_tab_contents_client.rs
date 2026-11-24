#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::api_values::{
    TryIntoApi, TryIntoEngine, stack_value,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::ActiveBuild;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::BuildKey;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::Contents;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::CreativeCache;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::EditSession;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::Published;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::api_stacks;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::comparison_key;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::display_name;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::list_items;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::snapshot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::target_lists;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    CreativeModeTabInfo, CreativeModeTabs, CreativeTabChange, CreativeTabEdit,
    CreativeTabEditError, CreativeTabList, CreativeTabPermissions, CreativeTabPlacement,
    CreativeTabReadError, CreativeTabVisibility,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::cell::{OnceCell, RefCell};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::atomic::{AtomicU64, Ordering};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_items::creative_catalog::{
    Catalog, Holders, Kind, Parameters, TabContents, normalize_components,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_items::items::Items;

impl Game<'_> {
    pub(in crate::game) fn build_creative(
        &mut self,
        permissions: CreativeTabPermissions,
    ) -> Result<Contents, CreativeTabReadError> {
        // A connection may exist in configuration without a live world provider.
        if self.native_lifecycle.epoch().is_none() {
            return Err(CreativeTabReadError::Unavailable);
        }
        let key = BuildKey {
            features: self.enabled_feature_ids().clone(),
            permissions,
        };
        if let Some(active) = &self.creative_contents.0.active {
            if active.key.matches(&key) {
                return Ok(active.partial.clone());
            }
            return Err(CreativeTabReadError::Reentrant);
        }
        if let Some(published) = &self.creative_contents.0.published
            && published.key.matches(&key)
        {
            return Ok(published.lists.clone());
        }
        let holders = self
            .creative_holders()
            .ok_or(CreativeTabReadError::Unavailable)?;
        let items = self.shared_items();
        let catalog = self.creative_screen.0.catalog().clone();
        let prior = self
            .creative_contents
            .0
            .published
            .as_ref()
            .map(|p| p.lists.clone());
        let listen = mod_api::__private::listened(mod_api::HookId::BuildCreativeModeTabContents);
        self.creative_contents.0.active = Some(ActiveBuild {
            key: BuildKey {
                features: key.features.clone(),
                permissions,
            },
            partial: Arc::from([]),
            items: items.clone(),
            edit: RefCell::new(None),
        });
        let lists = catalog.build(
            Parameters {
                features: &key.features,
                operator: permissions == CreativeTabPermissions::Operator,
                holders: &holders,
                items: &items,
            },
            prior.as_deref(),
            |tab, partial, current| {
                if listen {
                    static NEXT_EDIT: AtomicU64 = AtomicU64::new(0);
                    let token = mod_api::__private::creative_tab_edit(
                        NEXT_EDIT.fetch_add(1, Ordering::Relaxed),
                    );
                    let active = self
                        .creative_contents
                        .0
                        .active
                        .as_mut()
                        .expect("active creative build");
                    active.partial = Arc::from(partial);
                    *active.edit.get_mut() = Some(EditSession {
                        token,
                        contents: std::mem::take(current),
                    });
                    self.native_build_creative_mode_tab_contents(
                        &tab.id,
                        key.features.to_vec(),
                        permissions,
                        token,
                    );
                    *current = self
                        .creative_contents
                        .0
                        .active
                        .as_mut()
                        .expect("active creative build")
                        .edit
                        .get_mut()
                        .take()
                        .expect("active tab edit")
                        .contents;
                }
            },
        );
        self.creative_contents.0.active = None;
        self.creative_contents.0.published = Some(Published {
            key,
            lists: lists.clone(),
            api: OnceCell::new(),
            holders: Arc::new(holders),
            items,
        });
        Ok(lists)
    }
    /// The last completed build's catalog, for a reader that cannot build: unavailable outside
    /// a live world's phase, and `None` before the side has built once.
    pub(in crate::game) fn built_creative_tabs(
        &self,
    ) -> Result<Option<Arc<CreativeModeTabs>>, CreativeTabReadError> {
        if self.native_lifecycle.epoch().is_none() {
            return Err(CreativeTabReadError::Unavailable);
        }
        Ok(self
            .creative_contents
            .0
            .published
            .as_ref()
            .map(|published| {
                published
                    .api
                    .get_or_init(|| {
                        snapshot(
                            self.creative_screen.0.catalog(),
                            published.key.permissions,
                            &published.lists,
                        )
                    })
                    .clone()
            }))
    }
    /// The search tab's canonical display list (not the client-only UI additions).
    pub(in crate::game) fn creative_search_items(&mut self) -> Vec<ItemStack> {
        if self.native_lifecycle.epoch().is_none() {
            return Vec::new();
        }
        let Ok(lists) = self.build_creative(self.creative_permissions()) else {
            return Vec::new();
        };
        self.creative_screen
            .0
            .catalog()
            .search_display(&lists)
            .to_vec()
    }
    pub(in crate::game) fn creative_stack_key(&self, stack: &ItemStack) -> Option<ItemStack> {
        self.native_lifecycle.epoch()?;
        // Use the very same prototypes as the generator during a reentrant read.
        let items = self
            .creative_contents
            .0
            .active
            .as_ref()
            .map_or_else(|| self.items(), |active| &active.items);
        comparison_key(stack.clone(), items).ok()
    }
    pub(in crate::game) fn creative_tabs(
        &mut self,
        permissions: CreativeTabPermissions,
    ) -> Result<Arc<CreativeModeTabs>, CreativeTabReadError> {
        if self.native_lifecycle.epoch().is_none() {
            return Err(CreativeTabReadError::Unavailable);
        }
        let lists = self.build_creative(permissions)?;
        let catalog = self.creative_screen.0.catalog();
        if self.creative_contents.0.active.is_some() {
            return Ok(snapshot(catalog, permissions, &lists));
        }
        let published = self
            .creative_contents
            .0
            .published
            .as_ref()
            .expect("complete creative build");
        Ok(published
            .api
            .get_or_init(|| snapshot(catalog, permissions, &lists))
            .clone())
    }
    pub(in crate::game) fn edit_creative<T>(
        &self,
        edit: CreativeTabEdit,
        apply: impl FnOnce(&mut TabContents, &Items) -> Result<T, CreativeTabEditError>,
    ) -> Result<T, CreativeTabEditError> {
        let Some(active) = &self.creative_contents.0.active else {
            return Err(CreativeTabEditError::Stale);
        };
        let mut session = active
            .edit
            .try_borrow_mut()
            .map_err(|_| CreativeTabEditError::Stale)?;
        let session = session
            .as_mut()
            .filter(|s| s.token == edit)
            .ok_or(CreativeTabEditError::Stale)?;
        apply(&mut session.contents, &active.items)
    }
    /// The name components that replace the creative search's prepared name lines.
    pub(in crate::game) fn refresh_creative_search_names(&mut self) {
        // Only when the search tab's stacks were made again, not every creative tick.
        let generation = self.creative_screen.0.search_generation();
        if self.client.creative_search_named == Some(generation) {
            return;
        }
        self.client.creative_search_named = Some(generation);
        let names = self
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
        self.creative_screen.0.set_search_names(names);
    }
    /// The screen uses the completed cache, with the same holder definitions for search text.
    pub(in crate::game) fn refresh_creative_tabs(&mut self) {
        let Ok(lists) = self.build_creative(self.creative_permissions()) else {
            return;
        };
        if let Some(published) = &self.creative_contents.0.published
            && Arc::ptr_eq(&lists, &published.lists)
        {
            self.creative_screen.0.publish(
                lists,
                published.holders.clone(),
                published.items.clone(),
                &self.pack.names,
            );
            self.refresh_creative_search_names();
        }
    }
    pub(in crate::game) fn reset_creative_contents(&mut self) {
        self.creative_contents.0 = CreativeCache::default();
        self.creative_screen.0.reset_contents();
    }
    /// The built-in sorter is only a reader. A cold sort previews stock order
    /// without publishing anything or giving mods a creative callback.
    pub(in crate::game) fn stock_sort_order(&self) -> Vec<ItemStack> {
        let preview;
        let lists = if let Some(published) = &self.creative_contents.0.published {
            &published.lists
        } else {
            preview = self.creative_screen.0.catalog().build(
                Parameters {
                    features: self.enabled_feature_ids(),
                    operator: false,
                    holders: &self.creative_holders().unwrap_or_default(),
                    items: self.items(),
                },
                None,
                |_, _, _| {},
            );
            &preview
        };
        self.creative_screen.0.search_order_from(lists)
    }
}

#[cfg(test)]
#[path = "build_creative_mode_tab_contents/tests_client.rs"]
pub(in crate::game) mod tests;
