use super::*;

impl crate::game::ClientGame<'_> {
    /// `Player.interactOn`: Fail skips this interaction, not the hand loop.
    pub(in crate::game) fn entity_interaction(&mut self, hand: Hand, id: i32) -> Option<bool> {
        if self.common.spectator() || !mod_api::__private::listened(mod_api::HookId::EntityInteract) {
            return None;
        }
        let kind = self.client_entity_interaction_kind(id)?;
        let stack = self.common.with_hand(hand, |g| g.held().cloned());
        let mut result = None;
        self.native_entity_interact(hand, id, &kind, stack.as_ref(), &mut result);
        match result? {
            mod_api::InteractionResult::Pass | mod_api::InteractionResult::TryWithEmptyHand => None,
            answer => Some(self.common.with_hand(hand, |g| g.interaction_consumed(answer))),
        }
    }

    pub(in crate::game) fn client_item_interaction(
        &mut self,
        hand: Hand,
        stack: &ItemStack,
    ) -> Option<bool> {
        if let Some(answer) = self.common.right_click_item(hand, stack) {
            if answer == mod_api::InteractionResult::Success {
                self.common.with_hand(hand, Game::swing_arm);
            }
            return Some(answer.consumes_action());
        }
        self.common.with_hand(hand, |g| g.use_item_in_hand(stack))
    }
}

use sunrise_gameplay_common::game::native_hooks::base::GameHooks as _;
