//! The game's implementations of the mod API's service traits.
use crate::game::{Game, api_values::TryIntoEngine};
use mod_api::*;
use std::sync::Arc;
use sunrise_client::{
    lan::guest::Guest,
    server::{user_cache::UserCache, user_lists::NameAndId},
};

impl ClientSession for Game<'_> {
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
impl Commands for Game<'_> {
    fn refresh_commands(&mut self) {
        self.native_refresh_commands()
    }
    fn feedback(&mut self, line: Text) -> Result<(), mod_api::text::TextFormError> {
        self.native_feedback(sunrise_client_lib::api_text::to_serialized_engine(&line)?);
        Ok(())
    }
    fn run_command(&mut self, line: &str) -> bool {
        self.native_run_command(line)
    }
    fn run_command_with_output(&mut self, line: &str) -> mod_api::CommandResult {
        self.native_run_command_with_output(line)
    }
}
impl Levels for Game<'_> {
    fn list_levels(&self) -> Vec<LevelRef> {
        self.native_list_levels()
    }
}
impl Messages for Game<'_> {
    fn broadcast(&self, line: Text) -> Result<(), mod_api::text::TextFormError> {
        self.native_broadcast(sunrise_client_lib::api_text::to_serialized_engine(&line)?);
        Ok(())
    }
}
impl Players for Game<'_> {
    fn server_players(&self) -> Vec<PlayerRef> {
        self.native_server_players()
    }
    fn player_by_name(&self, name: &str) -> Option<PlayerRef> {
        self.native_player_by_name(name)
    }
    fn profile_by_uuid(&self, uuid: u128) -> Option<GameProfile<'_>> {
        self.lookup_profile(|player| player.uuid == uuid, |cache| cache.find_by_id(uuid))
    }
    fn profile_by_name(&self, name: &str) -> Option<GameProfile<'_>> {
        self.lookup_profile(
            |player| player.name.eq_ignore_ascii_case(name),
            |cache| cache.find_by_name(name),
        )
    }
    fn profile_permission_level(&self, uuid: u128, name: &str) -> PermissionLevel {
        if let Some(lists) = &self.server_lists {
            let lists = lists.lock().unwrap_or_else(|e| e.into_inner());
            return lists
                .ops
                .permission_level(uuid)
                .map_or(PermissionLevel::All, PermissionLevel::from_level);
        }
        let Some(host) = &self.lan else {
            return PermissionLevel::All;
        };
        let Some((owner, owner_uuid)) = &host.hub.lan.shared.config.singleplayer_profile else {
            return PermissionLevel::All;
        };
        let is_owner = owner.eq_ignore_ascii_case(name);
        let allow_commands = self.allow_commands();
        // IntegratedServer.getCustomPermissionLevel: for non-owners this takes
        // precedence over any entry, even when guest command access is off.
        if !is_owner && allow_commands {
            return if host.settings.guest_commands {
                PermissionLevel::Gamemasters
            } else {
                PermissionLevel::All
            };
        }
        // No persisted op list on integrated servers. The hub also stores
        // temporary LAN operator levels; its automatic 0 for the owner means
        // commands are off, not that the owner's profile is listed as an op.
        let op_level = host
            .hub
            .ops
            .get(&uuid)
            .copied()
            .filter(|&level| uuid != *owner_uuid || level != 0);
        // IntegratedServer.getProfilePermissions overrides an owner's level
        // when PlayerList.isOp is true, either by entry or by Allow Commands.
        if is_owner && (op_level.is_some() || allow_commands) {
            return PermissionLevel::Owners;
        }
        // MinecraftServer.getProfilePermissions: the op entry's level, or
        // ALL. The implicit-op case is already handled by the owner override.
        op_level.map_or(PermissionLevel::All, |level| {
            PermissionLevel::from_level(i32::from(level))
        })
    }
    fn acting_player(&self) -> Option<PlayerRef> {
        self.native_acting_player()
    }
}
impl Game<'_> {
    /// Borrow from the online records before consulting the offline cache.
    fn lookup_profile(
        &self,
        matches: impl Fn(&Guest) -> bool,
        cached: impl FnOnce(&UserCache) -> Option<&NameAndId>,
    ) -> Option<GameProfile<'_>> {
        if let Some(online) = self
            .lan
            .iter()
            .flat_map(|host| host.hub.online_players())
            .find(|player| matches(player))
        {
            return Some(mod_api::__private::game::game_profile(
                online.uuid,
                &online.name,
                &online.properties,
            ));
        }
        let cached = cached(self.user_cache.as_ref()?)?;
        Some(mod_api::__private::game::game_profile(
            cached.id,
            &cached.name,
            &[],
        ))
    }
}
impl RuntimeLevels for Game<'_> {
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
impl RenderAssets for Game<'_> {
    fn model(&self, id: &Identifier) -> Result<ModelRef, ModelError> {
        self.native_model(id)
    }
    fn block_state_models(&self, state: BlockState) -> Result<BakedModelKeys<'_>, ModelError> {
        let state = state
            .try_into_engine(&())
            .map_err(|_| ModelError::UnknownModel)?;
        Ok(self.pack.render.block_state_models(state))
    }
    fn block_state_model(&self, state: BlockState) -> Result<ModelRef, ModelError> {
        let state = state
            .try_into_engine(&())
            .map_err(|_| ModelError::UnknownModel)?;
        self.native_block_state_model(state)
    }
    fn baked_model(&self, key: BakedModelKey) -> Result<BakedModelView<'_>, ModelError> {
        self.pack.render.baked_model(key)
    }
    fn render_pipeline(&self, id: &Identifier) -> Result<RenderPipelineRef, RenderPipelineError> {
        self.pack.render.render_pipeline(id)
    }
    fn animation(&self, id: &Identifier) -> Option<AnimationHolder> {
        self.pack.render.animation(id)
    }
}
