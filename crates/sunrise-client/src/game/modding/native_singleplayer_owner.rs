//! `Players::singleplayer_profile` and `PlayerRef::is_singleplayer_owner`
//! (`MinecraftServer.getSingleplayerProfile`, `isSingleplayerOwner`): read from the profile
//! the integrated server launched with, which its login already keeps.
use super::Game;
use mod_api::{GameProfile, PlayerRef};

impl Game<'_> {
    /// `IntegratedServer.getSingleplayerProfile`: the launch profile's name and UUID, and the
    /// account's properties as far as the server's background fetch has them. None for a
    /// dedicated server, which launches with no such profile, and for a client.
    pub(super) fn native_singleplayer_profile(&self) -> Option<GameProfile<'_>> {
        let config = &self.lan.as_ref()?.hub.lan.shared.config;
        let (name, uuid) = config.singleplayer_profile.as_ref()?;
        let properties = config
            .owner_profile
            .as_deref()
            .map_or(&[][..], |owner| owner.properties());
        Some(mod_api::__private::game::game_profile(
            *uuid, name, properties,
        ))
    }

    /// `IntegratedServer.isSingleplayerOwner`: the connected player's name against the launch
    /// profile's, ignoring case; the UUID and permission level play no part.
    pub(super) fn player_is_singleplayer_owner(&self, player: PlayerRef) -> bool {
        let Some(guest) = self.online_player_record(player) else {
            return false;
        };
        self.native_singleplayer_profile()
            .is_some_and(|owner| owner.name.eq_ignore_ascii_case(&guest.name))
    }
}
