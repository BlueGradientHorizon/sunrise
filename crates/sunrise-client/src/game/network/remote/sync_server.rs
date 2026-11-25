#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::sync::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::tab_list::{TabPlayer, default_skin};

impl Remote {
    /// `ClientPacketListener.getPlayerInfo(String)`: exact profile name, whether listed or not.
    pub(in crate::game) fn player_info_by_name(&self, name: &str) -> Option<&TabPlayer> {
        self.tab_players.values().find(|p| p.name == name)
    }
}
