#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game,
    api_values::{project_stack, stack_read_metadata},
    commands::Target,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_item_entity_refs::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{EntityRef, ItemEntityRef, ItemStackView};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    items::ItemStack,
    lan::playerdata::{ints_uuid, uuid_ints},
};

impl Game<'_> {
    /// `ItemEntity.getOwner`: the thrower as a resolved entity, not the pickup
    /// target. None for no thrower, an unloaded or different-level thrower,
    /// an unavailable handle, or one Sunrise cannot resolve.
    pub(in crate::game) fn item_entity_owner(&self, item: ItemEntityRef) -> Option<EntityRef> {
        let uuid = self
            .dropped
            .get(self.native_item_entity_index(item, None)?)?
            .thrower?;
        let target = self.target_by_uuid(ints_uuid(uuid))?;
        if self.target_level(target) != self.dim {
            return None;
        }
        let id = u32::try_from(self.network_entity_id(target)?).ok()?;
        self.entity(id)
    }
}
