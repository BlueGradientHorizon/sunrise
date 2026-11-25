//! Common item-stack display name and rarity colour helpers.
use super::Game;
use sunrise_client::items::ItemStack;

impl Game<'_> {
    /// `ItemStack.getStyledHoverName`, without the rarity colour applied by the tooltip.
    pub(in crate::game) fn styled_stack_name(
        &self,
        stack: &ItemStack,
    ) -> sunrise_client::text::Text {
        let name = self.stack_hover_name_text(stack);
        if stack.data().is_some_and(|data| data.custom_name.is_some()) {
            sunrise_client::text::Text::empty().append(name).italic()
        } else {
            name
        }
    }

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
        match sunrise_client::item_tooltip::rarity(
            stack,
            props.map_or("common", |p| p.rarity.as_str()),
        ) {
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
