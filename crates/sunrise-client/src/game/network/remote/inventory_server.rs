#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::CREATIVE_SYNCED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::Predicted;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::changed_slots;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::creative_sync_slot;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::creative_sync_stacks;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::inventory_screen_slot;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::inventory_screen_slot_with_type;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::mark_synced;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::menu_items;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::open_hash_order;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::predict_clicks;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::predict_clicks_with;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::remote_station_kind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::*;

impl Game<'_> {
    // remote_send_state: remote/sync.rs
    /// `MultiPlayerGameMode.handleCreativeModeItemAdd` for inventory slot `i` (0 to 35) or the
    /// off hand (36): the stack goes out now and counts as synced. Nothing without a server.
    pub(in crate::game) fn remote_creative_slot_add(&mut self, i: usize) {
        let stack = if i == 36 {
            self.offhand.clone()
        } else {
            self.inventory.get(i).cloned().flatten()
        };
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        if let Some(synced) = r.synced.get_mut(i) {
            *synced = stack.clone();
        }
        self.send_creative_item_add(creative_sync_slot(i), stack);
    }
}
