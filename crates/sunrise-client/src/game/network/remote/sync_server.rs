use super::*;
use sunrise_client::tab_list::TabPlayer;

impl Remote {
    /// `ClientPacketListener.getPlayerInfo(String)`: exact profile name, whether listed or not.
    pub(in crate::game) fn player_info_by_name(&self, name: &str) -> Option<&TabPlayer> {
        self.tab_players.values().find(|p| p.name == name)
    }
}
