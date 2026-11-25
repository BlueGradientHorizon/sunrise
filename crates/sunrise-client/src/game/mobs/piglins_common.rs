//! Role-neutral piglin item queries shared by server AI and client interaction prediction.
use super::Game;
use sunrise_client::{items::ItemStack, mobs::piglin::BARTERING_ITEM};

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn piglin_loved(&self, item: &str) -> bool {
        self.item_in_tag("minecraft:piglin_loved", item)
    }

    /// `PiglinAi.isBarterCurrency`: vanilla's answer (`BARTERING_ITEM`), then item-part hooks.
    pub(in crate::game) fn piglin_currency(&self, stack: &ItemStack) -> bool {
        let mut currency = &*stack.item == BARTERING_ITEM;
        if mod_api::__private::item_listens(mod_api::HookId::IsPiglinCurrency, &stack.item) {
            self.native_is_piglin_currency(stack, &mut currency);
        }
        currency
    }
}
