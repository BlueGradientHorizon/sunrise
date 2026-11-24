#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::interaction::*;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn creative_item_enabled(
        &self,
        stack: &ItemStack,
        features: &[String],
    ) -> bool {
        self.creative_screen
            .0
            .catalog()
            .item_enabled(stack, features)
    }
}
