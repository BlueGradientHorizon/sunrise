use super::*;
use mod_api::SwimmingCheck;

impl crate::game::ServerGame<'_> {
    pub(in crate::game) fn server_ask_swim_eligibility(
        &self,
        check: SwimmingCheck,
    ) -> Option<bool> {
        let uuid = self.server.lan_acting_uuid.or_else(|| {
            self.common.level
                .player_uuid
                .map(sunrise_sim::lan::playerdata::ints_uuid)
        })?;
        let player = self.common.player(uuid)?;
        if Some(player.entity().id()) != self.common.player.as_ref().map(|p| p.entity_id as u32) {
            return None;
        }
        let mut answer = None;
        self.common.native_can_start_swimming(player, LogicalSide::Server, check, &mut answer);
        answer
    }
}
