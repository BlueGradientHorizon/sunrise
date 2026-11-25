#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::api_values::{
    TryIntoApi, TryIntoEngine, stack_value,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::ActiveBuild;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::BuildKey;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::Contents;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::CreativeCache;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::EditSession;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::Published;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::api_stacks;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::comparison_key;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::display_name;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::list_items;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::snapshot;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::target_lists;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    CreativeModeTabInfo, CreativeModeTabs, CreativeTabChange, CreativeTabEdit,
    CreativeTabEditError, CreativeTabList, CreativeTabPermissions, CreativeTabPlacement,
    CreativeTabReadError, CreativeTabVisibility,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::cell::{OnceCell, RefCell};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::atomic::{AtomicU64, Ordering};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_items::creative_catalog::{
    Catalog, Holders, Kind, Parameters, TabContents, normalize_components,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_items::items::Items;

impl Game<'_> {
    pub(in crate::game) fn server_creative_holders(&self) -> Option<Holders> {
        self.registry_sync().map(|sync| sync.creative_holders())
    }

    pub(in crate::game) fn server_creative_tab_entries(
        &self,
        edit: CreativeTabEdit,
        list: CreativeTabList,
    ) -> Result<Vec<ItemStack>, CreativeTabEditError> {
        // Active CreativeCache still lives in ClientOnly until world exposes it to both sides.
        self.client_creative_tab_entries(edit, list)
    }

    pub(in crate::game) fn server_creative_tab_change(
        &self,
        edit: CreativeTabEdit,
        change: CreativeTabChange,
    ) -> Result<(), CreativeTabEditError> {
        // Active CreativeCache still lives in ClientOnly until world exposes it to both sides.
        self.client_creative_tab_change(edit, change)
    }
}

#[cfg(test)]
#[path = "build_creative_mode_tab_contents/tests_server.rs"]
pub(in crate::game) mod tests;
