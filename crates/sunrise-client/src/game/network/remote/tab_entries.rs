//! The tab list's entries as the client holds them, for reads that must not change them.

use super::Remote;
use sunrise_client::tab_list::{TabPlayer, default_skin};

impl Remote {
    /// The listed players (`ClientPacketListener.getListedOnlinePlayers`) in encounter order.
    /// `tab_list::sorted_players` gives the overlay's display order.
    pub(in crate::game) fn listed_tab_player_refs(&self) -> impl Iterator<Item = &TabPlayer> {
        self.listed
            .iter()
            .filter_map(|uuid| self.tab_players.get(uuid))
    }
}

impl Remote {
    /// The listed players' skins as the Tab overlay draws them: each player's resolved skin
    /// texture path and whether it uses the slim model, in no order. A skin still downloading
    /// reads as the profile's default (`tab_list::default_skin`), as the overlay draws it.
    pub(in crate::game) fn listed_player_skins(&self) -> Vec<(u128, String, bool)> {
        self.listed
            .iter()
            .map(|uuid| {
                // The resolved skin's model is the one `update_remote_skins`
                // kept (`ResolvedSkin::body`): the textures' `slim`, plus the
                // local player's launcher offline skin and signed-in account
                // skin, which `skin_textures` never holds.
                let (skin, slim) = self
                    .skins
                    .get(uuid)
                    .map(|skin| skin.body(*uuid))
                    .unwrap_or_else(|| {
                        let skin = default_skin(*uuid);
                        (skin, skin.contains("/slim/"))
                    });
                (*uuid, skin.to_owned(), slim)
            })
            .collect()
    }
}
