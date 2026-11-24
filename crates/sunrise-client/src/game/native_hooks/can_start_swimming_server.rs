#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::can_start_swimming::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::SwimmingCheck;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{motion::SwimEntry, sprint};

impl Game<'_> {
    pub(in crate::game) fn ask_swim_eligibility(&self, check: SwimmingCheck) -> Option<bool> {
        let (player, side) = if self.remote.is_some() {
            (self.local_player()?, LogicalSide::Client)
        } else {
            let uuid = self.server.lan_acting_uuid.or_else(|| {
                self.level
                    .player_uuid
                    .map(sunrise_client::lan::playerdata::ints_uuid)
            })?;
            (self.player(uuid)?, LogicalSide::Server)
        };
        if Some(player.entity().id()) != self.player.as_ref().map(|p| p.entity_id as u32) {
            return None;
        }
        let mut answer = None;
        self.native_can_start_swimming(player, side, check, &mut answer);
        answer
    }
}
