use super::{Game, Hit, item_use_common};
use sunrise_items::items::ItemStack;

impl crate::game::ClientGame<'_> {
    /// Client-side `Consumable.startConsuming`: instant consumables await server sync.
    pub(in crate::game::client_side) fn client_start_eating(
        &mut self,
        info: &sunrise_items::items::ItemInfo,
    ) -> bool {
        let Some(stack) = self.common.held().cloned() else {
            return false;
        };
        let Some(consumable) = self.consumable(&stack) else {
            return false;
        };
        if sunrise_sim::item_sync::food(&stack, info.food).is_some()
            && (self.common.player.is_none()
                || !self.can_eat(
                    Some(&stack),
                    sunrise_sim::item_sync::can_always_eat(&stack, info.can_always_eat),
                ))
        {
            return false;
        }
        if item_use_common::consumes_at_once(consumable.ticks()) {
            return true;
        }
        self.start_using_item();
        true
    }
    pub(in crate::game) fn client_on_start_using_item(&mut self, _stack: &ItemStack) {}
    pub(in crate::game) fn client_item_pick(&self) -> Option<Hit> {
        let (origin, direction) = self.aim()?;
        self.clip_ray(origin, direction, self.common.block_reach)
    }
    pub(in crate::game) fn client_bucket_liquid_user_entity_id(&self) -> Option<u32> {
        (self.client.remote.is_some() || self.common.player.is_some()).then(|| self.own_entity_id() as u32)
    }
    pub(in crate::game) fn client_projectile_player_uuid(&mut self) -> Option<u128> {
        self.client.remote.as_ref().map(|r| r.profile())
    }
    pub(in crate::game) fn client_acting_player_ref(&self) -> Option<mod_api::PlayerRef> {
        self.local_player()
    }
    pub(in crate::game) fn client_current_level_ref(&self) -> Option<mod_api::LevelRef> {
        self.native_client_level()
    }
}

use sunrise_gameplay_common::game::GameGameExt1 as _;

use sunrise_gameplay_common::game::held_items::HeldItemsGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::pick_ray::PickRayGameExt1 as _;

use sunrise_gameplay_common::game::player_identity::PlayerIdentityGameExt1 as _;
