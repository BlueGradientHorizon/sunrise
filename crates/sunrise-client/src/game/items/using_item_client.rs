use super::{Game, Hit, item_use};
use sunrise_client::items::ItemStack;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Client-side `Consumable.startConsuming`: instant consumables await server sync.
    pub(in crate::game::client_side) fn client_start_eating(
        &mut self,
        info: &sunrise_client::items::ItemInfo,
    ) -> bool {
        let Some(stack) = self.held().cloned() else {
            return false;
        };
        let Some(consumable) = self.consumable(&stack) else {
            return false;
        };
        if sunrise_client::item_sync::food(&stack, info.food).is_some()
            && (self.player.is_none()
                || !self.can_eat(
                    Some(&stack),
                    sunrise_client::item_sync::can_always_eat(&stack, info.can_always_eat),
                ))
        {
            return false;
        }
        if item_use::consumes_at_once(consumable.ticks()) {
            return true;
        }
        self.start_using_item();
        true
    }
    pub(in crate::game) fn client_on_start_using_item(&mut self, _stack: &ItemStack) {}
    pub(in crate::game) fn client_item_pick(&self) -> Option<Hit> {
        let (origin, direction) = self.aim()?;
        self.clip_ray(origin, direction, self.block_reach)
    }
    pub(in crate::game) fn client_bucket_liquid_user_entity_id(&self) -> Option<u32> {
        (self.remote.is_some() || self.player.is_some()).then(|| self.own_entity_id() as u32)
    }
    pub(in crate::game) fn client_projectile_player_uuid(&mut self) -> Option<u128> {
        self.remote.as_ref().map(|r| r.profile())
    }
    pub(in crate::game) fn client_acting_player_ref(&self) -> Option<mod_api::PlayerRef> {
        self.local_player()
    }
    pub(in crate::game) fn client_current_level_ref(&self) -> Option<mod_api::LevelRef> {
        self.native_client_level()
    }
}
