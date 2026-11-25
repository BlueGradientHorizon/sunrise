#[allow(unused_imports)]
pub(in crate::game::client_side) use super::api_values::{TryIntoApi, TryIntoEngine};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Dim, Game, commands::execute, respawn_anchor::RespawnConfig,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::level_control_hooks::LevelControl;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::Acting;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::BaseTree;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::CommandExecutorSlots;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::FakeAction;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::LEVELS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::PlayerRead;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::ScopedPlayerState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::argument_kinds::{
    CommandArguments, command_arguments,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::argument_kinds::{
    UNCENTERED, argument_type, bounded_parser, custom_parser, custom_value, dimension_value,
    nbt_value, parsed_type, parser, positional_value, resource_parser,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::base_root;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::client_command_tree;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::command_tree;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::command_tree_for;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::declares_provider;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::finish;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::online_mode_of;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::orb_meets;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::ordered_box;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::register_command_nodes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::register_dispatcher;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::registered_commands::NativeCommandState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::system_chat;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::usage_nodes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_server::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::__private::game::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::LevelRef;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    ActionOutcome, ActionRefused, FakeEffect, GameMode, GlobalPos, Identifier, PlayerAction,
    PlayerActionSetup, PlayerFood, PlayerRef, PlayerRespawnConfig, ScopedActionResult, Sidebar,
    TextFormError, VirtualHost,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use serde_json::{Map, Value, json};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{
    LazyLock, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{collections::HashMap, path::PathBuf, sync::Arc};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    command::{Arg, ArgType, Context, Dispatcher},
    items::ItemStack,
    lan::{
        abilities::PlayerAbilities, data::component_nbt, guest::Guest, scoreboard::SidebarContents,
    },
    motion::PlayerMotion,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play::{self, Clientbound as C};

/// Helpers for server mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
#[allow(dead_code)]
impl Game<'_> {
    /// Reads live `Remote::abilities` with predicted `flying` and `flying_speed` from the body.
    pub(in crate::game) fn client_abilities(&self) -> Option<PlayerAbilities> {
        let mut a = *self.remote_abilities()?;
        if let Some(body) = self.player.as_ref() {
            a.flying = body.flying;
            a.flying_speed = body.flying_speed;
        }
        Some(a)
    }

    pub(in crate::game) fn client_player(&self, uuid: u128) -> Option<PlayerRef> {
        let epoch = self.native_lifecycle.epoch()?;
        let remote = self
            .remote
            .as_ref()
            .filter(|r| r.logged_in && r.profile == uuid)?;
        Some(mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(epoch, u32::try_from(remote.entity_id).ok()?),
            uuid,
        ))
    }

    pub(in crate::game) fn client_player_read_both(
        &self,
        player: PlayerRef,
    ) -> Option<PlayerRead<'_>> {
        (self.local_player()? == player).then(|| PlayerRead::Acting {
            mode: self
                .player_info_game_mode()
                .map(sunrise_client::command::args::GameMode::id),
            body: self.player.as_ref(),
            abilities: self.client_abilities(),
            last_death: self.level.last_death.as_ref(),
        })
    }

    pub(in crate::game) fn client_player_dimension_key(
        &self,
        player: PlayerRef,
    ) -> Option<mod_api::DimensionKey> {
        let epoch = self.native_lifecycle.epoch()?;
        if epoch != player.fields().entity.epoch() {
            return None;
        }
        (self.local_player()? == player).then(|| self.native_level_key())
    }

    pub(in crate::game) fn client_player_collision_flags(
        &self,
        player: PlayerRef,
    ) -> Result<Option<mod_api::CollisionFlags>, mod_api::LevelReadError> {
        Ok(self.client_player_read_both(player).and_then(|read| {
            let PlayerRead::Acting {
                body: Some(body), ..
            } = read
            else {
                return None;
            };
            let flags = body.collision;
            Some(mod_api::CollisionFlags::new(
                flags.horizontal,
                flags.minor,
                flags.vertical,
                flags.below,
            ))
        }))
    }

    pub(in crate::game) fn client_player_client_brand(&self, _player: PlayerRef) -> Option<&str> {
        None
    }

    pub(in crate::game) fn client_tick_uses_server_sections(&self) -> bool {
        false
    }

    pub(in crate::game) fn client_crafting_remainder_hook(
        &self,
        _stack: &ItemStack,
        _remainder: &mut Option<ItemStack>,
    ) {
    }

    pub(in crate::game) fn client_world_table_items(
        &self,
    ) -> Option<&Arc<sunrise_client::items::Items>> {
        None
    }

    pub(in crate::game) fn client_lid_openers(&self) -> Option<&HashMap<(i32, i32, i32), i32>> {
        None
    }

    pub(in crate::game) fn client_signal_capability_site(
        &self,
        _at: mod_api::BlockSide,
    ) -> Option<mod_api::BlockCapabilitySite> {
        None
    }

    pub(in crate::game) fn client_signal_container_size(
        &self,
        _at: mod_api::BlockSide,
    ) -> Option<usize> {
        None
    }

    pub(in crate::game) fn client_signal_container_stack(
        &self,
        _at: mod_api::BlockSide,
        _slot: usize,
    ) -> Option<mod_api::ItemStack> {
        None
    }

    pub(in crate::game) fn client_level_block_face_sturdy(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        face: mod_api::BlockFace,
        support: mod_api::SupportType,
    ) -> Option<bool> {
        crate::game::server_side::native_world_read::block_face_sturdy(
            self.client_level_view_world(level)?,
            self.res.behaviours(),
            self.res.collision(),
            pos,
            face,
            support,
        )
    }

    pub(in crate::game) fn client_signal_block_entity(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<mod_api::BlockEntityRef> {
        let world = self.client_level_view_world(level)?;
        crate::game::server_side::native_block_entity_refs::world_block_entity(world, level, pos)
    }

    pub(in crate::game) fn client_signal_block_entity_data(
        &self,
        entity: mod_api::BlockEntityRef,
    ) -> Option<mod_api::Compound> {
        let world = self.client_level_view_world(entity.fields().level)?;
        crate::game::server_side::native_block_entity_refs::world_block_entity_data(
            world,
            self.pack.assets.registry,
            self.items(),
            entity,
        )
    }

    pub(in crate::game) fn client_level_is_raining_at(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<bool> {
        let world = self.client_level_view_world(level)?;
        if !world.is_loaded_at(pos[0], pos[1], pos[2]) {
            return None;
        }
        let (rain, _) = crate::game::mod_api_host_common::weather_for_level(
            world.shape,
            level.dimension() == Dim::End.key(),
            world.weather.levels(1.0),
        );
        if rain <= 0.2 {
            return Some(false);
        }
        Some(crate::game::server_side::native_level_world::rain_at(
            world,
            self.res.facts(),
            self.world_biomes
                .as_deref()
                .unwrap_or_else(|| self.res.biomes()),
            self.res.behaviours(),
            Dim::from_id(level.dimension_id()),
            BlockPos::new(pos[0], pos[1], pos[2]),
        ))
    }
}
