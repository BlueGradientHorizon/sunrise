#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;

impl Game<'_, crate::game::client_side::ClientState> {
    /// `Player.interactOn`: Fail skips this interaction, not the hand loop.
    pub(in crate::game) fn entity_interaction(&mut self, hand: Hand, id: i32) -> Option<bool> {
        if self.spectator() || !mod_api::__private::listened(mod_api::HookId::EntityInteract) {
            return None;
        }
        let kind = self.client_entity_interaction_kind(id)?;
        let stack = self.with_hand(hand, |g| g.held().cloned());
        let mut result = None;
        self.native_entity_interact(hand, id, &kind, stack.as_ref(), &mut result);
        match result? {
            mod_api::InteractionResult::Pass | mod_api::InteractionResult::TryWithEmptyHand => None,
            answer => Some(self.with_hand(hand, |g| g.interaction_consumed(answer))),
        }
    }
}
