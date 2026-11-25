#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use sunrise_client::tab_list::{TabPlayer, default_skin};

impl Remote {
    /// `PlayerInfo.getSkin`: resolved skin or the profile's default, independent of listing.
    pub(in crate::game) fn player_info_skin(&self, uuid: u128) -> &str {
        self.skins
            .get(&uuid)
            .map(|skin| skin.body(uuid).0)
            .unwrap_or_else(|| default_skin(uuid))
    }
}

#[cfg(test)]
#[path = "team_packet_tests.rs"]
pub(in crate::game) mod team_packet_tests;

impl Game<'_> {
    // remote_effect: remote/effect.rs

    pub(in crate::game) fn remote_tab_players(
        &mut self,
    ) -> Option<(Vec<TabPlayer>, bool, Vec<u128>)> {
        self.update_remote_skins();
        self.publish_tab_object_skins();
        let r = self.remote.as_ref()?;
        let players: Vec<_> = r
            .listed
            .iter()
            .filter_map(|u| {
                let mut player = r.tab_players.get(u)?.clone();
                player.skin = r.player_info_skin(*u).to_owned();
                Some(player)
            })
            .collect();
        let upside_down = players
            .iter()
            .filter(|p| {
                (p.name == "Dinnerbone" || p.name == "Grumm")
                    && (p.uuid == r.profile || r.level_players.values().any(|u| *u == p.uuid))
            })
            .map(|p| p.uuid)
            .collect();
        Some((players, r.online_mode, upside_down))
    }
}
