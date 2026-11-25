//! The game's implementations of the mod API's service traits.
#[allow(unused_imports)]
use crate::game::{Game, api_values::TryIntoEngine};
#[allow(unused_imports)]
use mod_api::*;
#[allow(unused_imports)]
use std::sync::Arc;

impl<__Side: crate::game::Side> ClientSession for Game<'_, __Side> {
    fn client_server(&self) -> Option<ClientServer<'_>> {
        self.native_client_server()
    }
    fn client_language(&self) -> Option<Arc<str>> {
        self.native_client_language()
    }
    fn client_latency(&self) -> Option<u32> {
        self.native_client_latency()
    }
    fn client_tab_list(&self) -> Option<ClientTabList> {
        self.native_client_tab_list()
    }
    fn client_frame_stats(&self) -> Option<ClientFrameStats> {
        self.native_client_frame_stats()
    }
}
impl<__Side: crate::game::Side> Commands for Game<'_, __Side> {
    fn refresh_commands(&mut self) {
        self.native_refresh_commands()
    }
    fn feedback(
        &mut self,
        line: Text,
        broadcast_to_admins: bool,
    ) -> Result<(), mod_api::text::TextFormError> {
        self.native_feedback(
            sunrise_client_lib::api_text::to_serialized_engine(&line)?,
            broadcast_to_admins,
        );
        Ok(())
    }
    fn failure(&mut self, line: Text) -> Result<(), mod_api::text::TextFormError> {
        self.native_failure(sunrise_client_lib::api_text::to_serialized_engine(&line)?);
        Ok(())
    }
    fn run_command(&mut self, line: &str) -> bool {
        self.native_run_command(line)
    }
    fn run_command_with_output(&mut self, line: &str) -> mod_api::CommandResult {
        self.native_run_command_with_output(line)
    }
}
impl<__Side: crate::game::Side> Levels for Game<'_, __Side> {
    fn list_levels(&self) -> Vec<LevelRef> {
        self.native_list_levels()
    }
}
impl<__Side: crate::game::Side> Messages for Game<'_, __Side> {
    fn broadcast(&self, line: Text) -> Result<(), mod_api::text::TextFormError> {
        self.native_broadcast(sunrise_client_lib::api_text::to_serialized_engine(&line)?);
        Ok(())
    }
}
impl<__Side: crate::game::Side> Players for Game<'_, __Side> {
    fn server_players(&self) -> Vec<PlayerRef> {
        self.native_server_players()
    }
    fn player_by_name(&self, name: &str) -> Option<PlayerRef> {
        self.native_player_by_name(name)
    }
    fn acting_player(&self) -> Option<PlayerRef> {
        self.native_acting_player()
    }
    fn singleplayer_profile(&self) -> Option<GameProfile<'_>> {
        self.native_singleplayer_profile()
    }
}
impl<__Side: crate::game::Side> Profiles for Game<'_, __Side> {
    fn resolve_name(&self, name: &str) -> ProfileResolution {
        self.native_resolve_profile(sunrise_client::skins::NameOrId::Name(name.to_owned()))
    }
    fn resolve_id(&self, uuid: u128) -> ProfileResolution {
        self.native_resolve_profile(sunrise_client::skins::NameOrId::Id(uuid))
    }
}
impl<__Side: crate::game::Side> RuntimeLevels for Game<'_, __Side> {
    fn open(&mut self, id: &str, level: RuntimeLevel) -> Result<LevelRef, RuntimeLevelError> {
        self.runtime_level_open(id, level)
    }
    fn unload(&mut self, id: &str) -> Result<(), RuntimeLevelError> {
        self.runtime_level_unload(id)
    }
    fn delete(&mut self, id: &str) -> Result<(), RuntimeLevelError> {
        self.runtime_level_delete(id)
    }
    fn is_open(&mut self, id: &str) -> bool {
        self.runtime_level_is_open(id)
    }
}
