use super::*;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game::server_side) fn use_item_fallthrough(
        &mut self,
        stack: &ItemStack,
        on_block: bool,
    ) -> Option<bool> {
        if on_block {
            return None;
        }
        if let Some(answer) = self.right_click_item(self.hand, stack) {
            if answer == mod_api::InteractionResult::SuccessServer {
                self.swing_arm();
            }
            return Some(answer.consumes_action());
        }
        self.use_item_in_hand(stack)
    }
    /// Finish all useOn handlers, including placement, before entering Item.use.
    pub(in crate::game::server_side) fn use_target_then_item(
        &mut self,
        hit: &crate::game::Hit,
    ) -> sunrise_client::Result<bool> {
        self.side.guest_use = Some(crate::game::GuestUse::on_block(self.hand));
        let result = self.with_block_use_original(self.hand, |g| {
            let result = g.use_hand(Some(hit));
            if matches!(result, Ok(true)) && !g.side.use_failed {
                g.finish_block_use(hit.pos, g.hand);
            }
            result
        });
        self.side.guest_use = None;
        if result? {
            return Ok(true);
        }
        self.use_hand(None)
    }
}
