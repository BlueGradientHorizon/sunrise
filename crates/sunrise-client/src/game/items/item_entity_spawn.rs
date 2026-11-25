//! `new ItemEntity(..)` and `ServerLevel.addFreshEntity` of a dropped item, kept
//! apart so `item_toss` can hear the item between the two.
#[allow(unused_imports)]
use super::{Game, spill};
#[allow(unused_imports)]
use sunrise_client::{entities::ItemEntity, items::ItemStack};
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;

impl Game<'_> {
    /// `ServerLevel.addFreshEntity` of the item `id` that `push_item_entity`
    /// made, with the stack it holds now.
    /// Whether the item entity is in the level after its join: false when a join listener kept
    /// it out (or no such item entity was made).
    pub(in crate::game) fn join_item_entity(&mut self, id: u32, position: [f64; 3]) -> bool {
        self.announce_item_entity(id, position, false)
    }

    pub(in crate::game) fn join_removal_item(&mut self, id: u32, position: [f64; 3]) {
        self.announce_item_entity(id, position, true);
    }

    pub(in crate::game) fn announce_item_entity(
        &mut self,
        id: u32,
        position: [f64; 3],
        in_removal: bool,
    ) -> bool {
        let Some((item, count)) = self
            .dropped
            .iter()
            .rev()
            .find(|d| d.id == id)
            .map(|d| (d.stack.item.clone(), d.stack.count as i32))
        else {
            return false;
        };
        if self.is_logical_server() && !mod_api::__private::mods().is_empty() {
            self.native_lifecycle
                .pending_item_spawns
                .push((id, item.to_string(), count, position));
            if in_removal {
                self.flush_removal_join(id)
            } else {
                self.native_joins_flush(false, id)
            }
        } else if self.native_joins_flush(false, id) {
            self.native_item_spawn(id, &item, count, position);
            true
        } else {
            false
        }
    }
}
