//! One mod-created property-set generation shared by Host and menu prediction.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

impl Game<'_> {
    /// Mod-created recipe input membership in the current owning generation.
    pub(in crate::game) fn mod_recipe_property_contains(
        &self,
        set: &str,
        item: mod_api::ItemKey,
    ) -> bool {
        mod_api::__private::item_key_id(item)
            .is_some_and(|name| self.loop_context.mod_recipe_property_contains(set, name))
    }

    #[inline(always)]
    pub(in crate::game) fn has_world_tables(&self) -> bool {
        self.world_tables.0.is_some()
    }
}
