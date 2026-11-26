//! Common item-stack display name and rarity colour helpers.
use crate::game::CommonGame;
use sunrise_items::items::ItemStack;

impl CommonGame<'_> {
    /// `ItemStack.getHoverName` as a component: custom name, book title, mod part, else vanilla.
    pub fn stack_hover_name_text(
        &self,
        stack: &ItemStack,
    ) -> sunrise_game_data::text::Text {
        sunrise_sim::stations::hover_name_in(&self.loop_context, &self.pack.names, stack)
    }

    /// `ItemStack.getItemName` as a component, without custom name or book title.
    pub(in crate::game) fn stack_item_name_text(
        &self,
        stack: &ItemStack,
    ) -> sunrise_game_data::text::Text {
        sunrise_sim::stations::item_name_in(&self.loop_context, &self.pack.names, stack)
    }

    /// `ItemStack.getRarity().color()` in text's RGB format.
    pub fn stack_rarity_color(&self, stack: &ItemStack) -> u32 {
        use sunrise_game_data::text;
        let props = self.stations().enchantments.props(&stack.item);
        match sunrise_sim::stations::rarity(stack, props.map_or("common", |p| p.rarity.as_str())) {
            "uncommon" => text::YELLOW,
            "rare" => text::AQUA,
            "epic" => text::LIGHT_PURPLE,
            _ => text::WHITE,
        }
    }

    /// The resolved `ItemStack.getHoverName` rendered as plain text.
    pub fn stack_hover_name(&self, stack: &ItemStack) -> String {
        self.stack_hover_name_text(stack).plain()
    }
}

use crate::game::tables::GameItems as _;
