//! One demand-driven creative catalog per logical side, and the scoped edit event.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::api_values::{TryIntoApi, TryIntoEngine, stack_value};
#[allow(unused_imports)]
use mod_api::{
    CreativeModeTabInfo, CreativeModeTabs, CreativeTabChange, CreativeTabEdit,
    CreativeTabEditError, CreativeTabList, CreativeTabPermissions, CreativeTabPlacement,
    CreativeTabReadError, CreativeTabVisibility,
};
#[allow(unused_imports)]
use std::cell::{OnceCell, RefCell};
#[allow(unused_imports)]
use std::sync::atomic::{AtomicU64, Ordering};
#[allow(unused_imports)]
use sunrise_items::creative_catalog::{
    Catalog, Holders, Kind, Parameters, TabContents, normalize_components,
};

#[allow(unused_imports)]
use sunrise_items::items::Items;

pub(in crate::game) type Contents = Arc<[Arc<TabContents>]>;

pub(in crate::game) struct BuildKey {
    pub(in crate::game) features: Arc<[String]>,
    pub(in crate::game) permissions: CreativeTabPermissions,
}

impl BuildKey {
    pub(in crate::game) fn matches(&self, other: &Self) -> bool {
        self.permissions == other.permissions
            && (self.features == other.features
                || (other
                    .features
                    .iter()
                    .all(|flag| self.features.contains(flag))
                    && self
                        .features
                        .iter()
                        .all(|flag| other.features.contains(flag))))
    }
}

pub(in crate::game) struct Published {
    pub(in crate::game) key: BuildKey,
    pub(in crate::game) lists: Contents,
    pub(in crate::game) api: OnceCell<Arc<CreativeModeTabs>>,
    pub(in crate::game) holders: Arc<Holders>,
    pub(in crate::game) items: Arc<Items>,
}

pub(in crate::game) struct ActiveBuild {
    pub(in crate::game) key: BuildKey,
    pub(in crate::game) partial: Contents,
    pub(in crate::game) items: Arc<Items>,
    pub(in crate::game) edit: RefCell<Option<EditSession>>,
}

pub(in crate::game) struct EditSession {
    pub(in crate::game) token: CreativeTabEdit,
    pub(in crate::game) contents: TabContents,
}

/// Replaced with the holder provider, never with tags or presentation. Thus the
/// cache key need not carry a registry pointer that changes on a tag-only update.
#[derive(Default)]
pub(in crate::game) struct CreativeCache {
    pub(in crate::game) published: Option<Published>,
    pub(in crate::game) active: Option<ActiveBuild>,
}

impl Game<'_> {
    /// Fire after one tab's generator with the live edit session installed.
    pub(in crate::game) fn native_build_creative_mode_tab_contents(
        &mut self,
        tab: &str,
        features: Vec<String>,
        permissions: mod_api::CreativeTabPermissions,
        edit: mod_api::CreativeTabEdit,
    ) {
        self.fire_build_creative_mode_tab_contents(tab, features, permissions, edit);
    }

    pub(in crate::game) fn creative_permissions(&self) -> CreativeTabPermissions {
        if self.can_use_game_master_blocks() && self.settings.operator_items_tab {
            CreativeTabPermissions::Operator
        } else {
            CreativeTabPermissions::Normal
        }
    }

    pub(in crate::game) fn creative_holders(&self) -> Option<Holders> {
        match &self.remote {
            Some(remote) => Some(sunrise_client::creative::live_holders(
                &remote.builtin,
                &remote.registries,
            )),
            None => self.registry_sync().map(|sync| sync.creative_holders()),
        }
    }

    pub(in crate::game) fn creative_tab_entries(
        &self,
        edit: CreativeTabEdit,
        list: CreativeTabList,
    ) -> Result<Vec<ItemStack>, CreativeTabEditError> {
        self.edit_creative(edit, |contents, _| Ok(list_items(contents, list).clone()))
    }

    pub(in crate::game) fn creative_tab_change(
        &self,
        edit: CreativeTabEdit,
        change: CreativeTabChange,
    ) -> Result<(), CreativeTabEditError> {
        match change {
            CreativeTabChange::Insert {
                placement,
                stack,
                visibility,
            } => self.creative_tab_insert(edit, placement, stack, visibility),
            CreativeTabChange::Remove { stack, visibility } => {
                self.creative_tab_remove(edit, &stack, visibility)
            }
        }
    }

    pub(in crate::game) fn creative_tab_insert(
        &self,
        edit: CreativeTabEdit,
        placement: CreativeTabPlacement,
        stack: mod_api::ItemStack,
        visibility: CreativeTabVisibility,
    ) -> Result<(), CreativeTabEditError> {
        self.edit_creative(edit, |contents, items| {
            if stack.count() != 1 {
                return Err(CreativeTabEditError::Count);
            }
            let mut stack = stack.try_into_engine(items)?;
            normalize_components(&mut stack, items);
            let (anchor, after) = match &placement {
                CreativeTabPlacement::Before(anchor) => {
                    (Some(comparison_key(stack_value(anchor)?, items)?), false)
                }
                CreativeTabPlacement::After(anchor) => {
                    (Some(comparison_key(stack_value(anchor)?, items)?), true)
                }
                _ => (None, false),
            };
            for list in target_lists(visibility) {
                let entries = list_items(contents, *list);
                if entries
                    .iter()
                    .any(|entry| entry.same_item_same_components(&stack))
                {
                    return Err(CreativeTabEditError::Duplicate(*list));
                }
                let index = match &anchor {
                    Some(anchor) => {
                        entries
                            .iter()
                            .position(|entry| entry.same_item_same_components(anchor))
                            .ok_or(CreativeTabEditError::MissingAnchor(*list))?
                            + usize::from(after)
                    }
                    None if matches!(placement, CreativeTabPlacement::First) => 0,
                    None => entries.len(),
                };
                entries.insert(index, stack.clone());
            }
            Ok(())
        })
    }

    pub(in crate::game) fn creative_tab_remove(
        &self,
        edit: CreativeTabEdit,
        stack: &mod_api::ItemStack,
        visibility: CreativeTabVisibility,
    ) -> Result<(), CreativeTabEditError> {
        self.edit_creative(edit, |contents, items| {
            let stack = comparison_key(stack_value(stack)?, items)?;
            for list in target_lists(visibility) {
                list_items(contents, *list)
                    .retain(|entry| !entry.same_item_same_components(&stack));
            }
            Ok(())
        })
    }
}

pub(in crate::game) fn api_stacks(stacks: &[ItemStack]) -> Vec<mod_api::ItemStack> {
    stacks
        .iter()
        .filter_map(|item| item.try_into_api().ok())
        .collect()
}

pub(in crate::game) fn snapshot(
    catalog: &Catalog,
    permissions: CreativeTabPermissions,
    lists: &[Arc<TabContents>],
) -> Arc<CreativeModeTabs> {
    let tabs = catalog
        .tabs()
        .iter()
        .zip(lists)
        .map(|(tab, stacks)| {
            CreativeModeTabInfo::new(
                tab.id.clone(),
                tab.kind == Kind::Category,
                display_name(&tab.id, &tab.title),
                api_stacks(&stacks.display),
                api_stacks(&stacks.search),
            )
        })
        .collect();
    Arc::new(CreativeModeTabs::new(permissions, tabs))
}

/// `CreativeModeTab.getDisplayName`: the tab's `itemGroup.*` key with its title as fallback.
pub(in crate::game) fn display_name(id: &str, title: &str) -> mod_api::Text {
    let key = sunrise_client::creative::item_group_key(id).map_or_else(
        || sunrise_client::creative::mod_item_group_key(id),
        str::to_owned,
    );
    mod_api::Text::new(mod_api::text::TextContent::Translatable {
        key,
        fallback: Some(title.to_owned()),
        arguments: Vec::new(),
    })
}

// Comparison ignores count and resident maxima, but damage must fit the component codec.
pub(in crate::game) fn comparison_key(
    mut stack: ItemStack,
    items: &Items,
) -> Result<ItemStack, mod_api::StackWriteError> {
    if i32::try_from(stack.damage).is_err() {
        return Err(mod_api::StackWriteError::InvalidComponents {
            reason: mod_api::StackError::InvalidDamage,
        });
    }
    normalize_components(&mut stack, items);
    Ok(stack)
}

pub(in crate::game) fn target_lists(
    visibility: CreativeTabVisibility,
) -> &'static [CreativeTabList] {
    use CreativeTabList::{Parent, Search};
    match visibility {
        CreativeTabVisibility::Parent => &[Parent],
        CreativeTabVisibility::Search => &[Search],
        CreativeTabVisibility::Both => &[Parent, Search],
    }
}

pub(in crate::game) fn list_items(
    contents: &mut TabContents,
    list: CreativeTabList,
) -> &mut Vec<ItemStack> {
    match list {
        CreativeTabList::Parent => &mut contents.display,
        CreativeTabList::Search => &mut contents.search,
    }
}

#[cfg(test)]
#[path = "build_creative_mode_tab_contents/tests.rs"]
pub(in crate::game) mod tests;
