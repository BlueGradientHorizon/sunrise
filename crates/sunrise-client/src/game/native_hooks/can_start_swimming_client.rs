#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::SwimmingCheck;

impl Game<'_> {
    pub(in crate::game) fn client_ask_swim_eligibility(
        &self,
        check: SwimmingCheck,
    ) -> Option<bool> {
        let player = self.local_player()?;
        if Some(player.entity().id()) != self.player.as_ref().map(|p| p.entity_id as u32) {
            return None;
        }
        let mut answer = None;
        self.native_can_start_swimming(player, LogicalSide::Client, check, &mut answer);
        answer
    }
}
