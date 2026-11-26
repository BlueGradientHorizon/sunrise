//! Role-neutral piglin item queries shared by server AI and client interaction prediction.
use crate::game::CommonGame;
use sunrise_items::items::ItemStack;
use sunrise_sim::mobs::piglin::BARTERING_ITEM;

impl crate::game::CommonGame<'_> {

    /// `PiglinAi.isBarterCurrency`: vanilla's answer (`BARTERING_ITEM`), then item-part hooks.
    pub fn piglin_currency(&self, stack: &ItemStack) -> bool {
        let mut currency = &*stack.item == BARTERING_ITEM;
        if mod_api::__private::item_listens(mod_api::HookId::IsPiglinCurrency, &stack.item) {
            self.native_is_piglin_currency(stack, &mut currency);
        }
        currency
    }
}

pub trait PiglinsCommonGameExt1<'__sg>: crate::game::SideGame<'__sg> {
     fn piglin_loved(&self, item: &str) -> bool {
        self.item_in_tag("minecraft:piglin_loved", item)
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> PiglinsCommonGameExt1<'__sg> for G {}


use crate::game::tables::WorldTablesExt as _;
