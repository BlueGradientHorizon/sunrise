#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::stack_names::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::ItemStack;

impl Game<'_, crate::game::client_side::ClientState> {
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
}
