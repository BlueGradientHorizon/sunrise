
use crate::game::ServerGame;
impl Commands for ServerGame<'_> {
    fn refresh_commands(&mut self) {
        self.native_refresh_commands()
    }
    fn feedback(
        &mut self,
        line: Text,
        broadcast_to_admins: bool,
    ) -> Result<(), mod_api::text::TextFormError> {
        self.native_feedback(
            sunrise_sim::api_text::to_serialized_engine(&line)?,
            broadcast_to_admins,
        );
        Ok(())
    }
    fn failure(&mut self, line: Text) -> Result<(), mod_api::text::TextFormError> {
        self.native_failure(sunrise_sim::api_text::to_serialized_engine(&line)?);
        Ok(())
    }
    fn run_command(&mut self, line: &str) -> bool {
        self.native_run_command(line)
    }
    fn run_command_with_output(&mut self, line: &str) -> mod_api::CommandResult {
        self.native_run_command_with_output(line)
    }
}
impl Levels for ServerGame<'_> {
    fn list_levels(&self) -> Vec<LevelRef> {
        self.native_list_levels()
    }
}
impl Messages for ServerGame<'_> {
    fn broadcast(&self, line: Text) -> Result<(), mod_api::text::TextFormError> {
        self.native_broadcast(sunrise_sim::api_text::to_serialized_engine(&line)?);
        Ok(())
    }
}
impl Players for ServerGame<'_> {
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
impl Profiles for ServerGame<'_> {
    fn resolve_name(&self, name: &str) -> ProfileResolution {
        self.native_resolve_profile(sunrise_level::profile_lookup::NameOrId::Name(
            name.to_owned(),
        ))
    }
    fn resolve_id(&self, uuid: u128) -> ProfileResolution {
        self.native_resolve_profile(sunrise_level::profile_lookup::NameOrId::Id(uuid))
    }
}
impl RuntimeLevels for ServerGame<'_> {
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