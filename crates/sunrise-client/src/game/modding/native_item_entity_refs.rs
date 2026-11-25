//! Dropped-item narrowing entry point for native mods (`EntityRef -> ItemEntityRef`).
use super::Game;
use mod_api::{EntityRef, ItemEntityRef};

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// Narrows an entity to a dropped item, or None for another kind, a
    /// stale handle, a client, or an item temporarily taken out for ticking.
    pub(in crate::game) fn item_entity(&self, entity: EntityRef) -> Option<ItemEntityRef> {
        self.side_item_entity(entity)
    }
}
