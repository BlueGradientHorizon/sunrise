#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::item_picked_up::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::entities::ItemEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::join_seq::JoinList;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::RandomSource;

impl Game<'_> {
    /// The take animation of `item` toward the host's player, and its sound at `feet`.
    pub(in crate::game) fn native_take_item(&mut self, item: ItemEntity, feet: [f64; 3]) {
        self.add_pickup(
            crate::game::pickups::Taken::Item(Box::new(item)),
            crate::game::pickups::Collector::Player,
        );
        // handleTakeItemEntity: the random pitch of a full pickup.
        // `ClientPacketListener.random`, the listener's own stream, never the level's.
        let mut random = sunrise_client::random_source::create();
        let pitch = ((random.next_float() - random.next_float()) * 0.7 + 1.0) * 2.0;
        self.audio
            .0
            .play("minecraft:entity.item.pickup", Some(feet), 0.2, pitch);
    }
}
