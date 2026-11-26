//! Dropped-item narrowing entry point for native mods (`EntityRef -> ItemEntityRef`).
use crate::game::CommonGame;
use mod_api::{EntityRef, ItemEntityRef};

impl CommonGame<'_> {
    /// Narrows an entity to a dropped item, or None for another kind, a
    /// stale handle, a client, or an item temporarily taken out for ticking.
    pub fn item_entity(&self, entity: EntityRef) -> Option<ItemEntityRef> {
        self.side_item_entity(entity)
    }
}

use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;
