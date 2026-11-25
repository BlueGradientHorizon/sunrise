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
    #[inline(always)]
    pub(in crate::game) fn cleanup_fake_player_client(&mut self, entity_id: i32) {
        self.client.destroying.remove(&entity_id);
    }
    /// Reads live `Remote::abilities` with predicted `flying` and `flying_speed` from the body.
    pub(in crate::game) fn client_abilities(&self) -> Option<PlayerAbilities> {
        let mut a = *self.remote_abilities()?;
        if let Some(body) = self.player.as_ref() {
            a.flying = body.flying;
            a.flying_speed = body.flying_speed;
        }
        Some(a)
    }
}
