use super::{Game, item_use};

impl Game<'_> {
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
}
