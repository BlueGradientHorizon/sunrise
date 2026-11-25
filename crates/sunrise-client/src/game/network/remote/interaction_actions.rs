#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    pub(in crate::game) fn remote_use(&mut self, attempt: mod_api::InteractionAttempt) {
        self.with_interaction_swing_scope(|game| game.remote_use_inner(attempt));
    }
    pub(in crate::game) fn remote_start_attack(&mut self) -> bool {
        // An agent's `attack ID` checks its entity against the pick its click is taken with,
        // and its click on another target is dropped before it acts.
        if self.agent_attack_click_refused() {
            return false;
        }
        self.with_interaction_swing_scope(|game| game.remote_start_attack_inner())
    }
    pub(in crate::game) fn remote_continue_attack(&mut self, down: bool) {
        self.with_interaction_swing_scope(|game| game.remote_continue_attack_inner(down));
    }
    /// `MultiPlayerGameMode.releaseUsingItem`: sends `RELEASE_USE_ITEM` and
    /// runs the client half of `Item.releaseUsing` (a Riptide launch moves the
    /// player itself). Vanilla reaches it both when the use key is let go and,
    /// in the same tick, when a keybind opens a screen.
    pub(in crate::game) fn release_remote_use(&mut self) {
        self.send_action(6);
        // a mod's Stop may skip the item's release, and a mod's own release follows.
        let hand = self.used_hand();
        if let Some((slot, ticks)) = self.using.take()
            && let Some(hand) = hand
            && let Some(stack) = self.stack_in(slot).cloned()
        {
            self.with_hand(hand, |g| {
                if !g.use_item_stopped(&stack, ticks) {
                    return;
                }
                if &*stack.item == "minecraft:trident" {
                    g.release_trident(g.ticks_held(&stack, ticks));
                }
                g.mod_release_using(&stack, ticks);
            });
        }
    }
}
