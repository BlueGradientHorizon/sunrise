//! Active item ownership retained across a mutable listener dispatch.
use super::Items;
use std::{ops::Deref, sync::Arc};

/// Retain the selected owner without copying its item registry.
pub(in crate::game) enum ItemSnapshot {
    Shared(Arc<Items>),
    Resources(Arc<super::Resources>),
}

impl ItemSnapshot {
    pub(in crate::game) fn capture(
        server: Option<&Arc<Items>>,
        world: Option<&Arc<Items>>,
        base: impl FnOnce() -> Self,
    ) -> Self {
        match server.or(world) {
            Some(items) => Self::Shared(Arc::clone(items)),
            None => base(),
        }
    }
}

impl Deref for ItemSnapshot {
    type Target = Items;
    fn deref(&self) -> &Items {
        match self {
            Self::Shared(items) => items,
            Self::Resources(resources) => resources.items(),
        }
    }
}

impl super::Game<'_> {
    pub(in crate::game) fn stack_boundary_items(&self) -> Option<ItemSnapshot> {
        Some(ItemSnapshot::capture(
            self.server_items.as_ref(),
            self.world_tables.0.as_ref().map(|tables| &tables.items),
            || ItemSnapshot::Resources(Arc::clone(&self.res)),
        ))
    }
}

impl<'a> super::GameLevel<'a> {
    pub(in crate::game) fn stack_boundary_items(&self) -> Option<&'a Items> {
        Some(self.items)
    }
}

/// Registry access for the remaining generated value conversions.
pub(in crate::game) trait ItemBoundary {
    fn boundary_items(&self) -> Option<&Items>;
}

impl ItemBoundary for super::Game<'_> {
    fn boundary_items(&self) -> Option<&Items> {
        Some(self.items())
    }
}

impl ItemBoundary for super::GameLevel<'_> {
    fn boundary_items(&self) -> Option<&Items> {
        Some(self.items)
    }
}

impl ItemBoundary for super::native_hooks::HitLevel<'_> {
    fn boundary_items(&self) -> Option<&Items> {
        self.0.tags()
    }
}
