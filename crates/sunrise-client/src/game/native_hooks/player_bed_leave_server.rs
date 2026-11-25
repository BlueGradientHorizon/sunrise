#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_bed_leave::WAKING;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_bed_leave::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::cell::RefCell;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// [`Self::native_player_bed_leave`] for the player acting now, who sleeps
    /// in `self.sleeping`; its name is copied only when a mod listens.
    pub(in crate::game::server_side) fn native_acting_bed_leave(
        &mut self,
        wake_immediately: bool,
        update_level: bool,
    ) {
        if let (Some(head), Some(player)) = (self.sleeping, self.server.lan_acting_uuid)
            && mod_api::__private::listened(mod_api::HookId::PlayerBedLeave)
        {
            let name = self.server.lan_acting_name.clone().unwrap_or_default();
            self.native_player_bed_leave(player, &name, head, wake_immediately, update_level);
        }
    }
}
