//! Common item-stack display name and rarity colour helpers.
use super::Game;
use sunrise_client::items::ItemStack;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// `ItemStack.getHoverName` as a component: custom name, book title, mod part, else vanilla.
    pub(in crate::game) fn stack_hover_name_text(
        &self,
        stack: &ItemStack,
    ) -> sunrise_client::text::Text {
        sunrise_client::stations::hover_name_in(&self.loop_context, &self.pack.names, stack)
    }

    /// `ItemStack.getItemName` as a component, without custom name or book title.
    pub(in crate::game) fn stack_item_name_text(
        &self,
        stack: &ItemStack,
    ) -> sunrise_client::text::Text {
        sunrise_client::stations::item_name_in(&self.loop_context, &self.pack.names, stack)
    }

    /// `ItemStack.getRarity().color()` in text's RGB format.
    pub(in crate::game) fn stack_rarity_color(&self, stack: &ItemStack) -> u32 {
        use sunrise_client::text;
        let props = self.stations().enchantments.props(&stack.item);
        match sunrise_client::stations::rarity(stack, props.map_or("common", |p| p.rarity.as_str()))
        {
            "uncommon" => text::YELLOW,
            "rare" => text::AQUA,
            "epic" => text::LIGHT_PURPLE,
            _ => text::WHITE,
        }
    }

    /// The resolved `ItemStack.getHoverName` rendered as plain text.
    pub(in crate::game) fn stack_hover_name(&self, stack: &ItemStack) -> String {
        self.stack_hover_name_text(stack).plain()
    }
}
