#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::BLOCK_PROPERTIES;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::BlockProperties;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::COMMAND_EXPORTS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::PLAYER_NAME;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::RESOURCE_IDS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::ResourceIds;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::block_properties;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::command_export;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::ids_in;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::json_ids_in;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::json_ids_of_every_namespace;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::overworld_timelines;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::owner_permission;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::resource_ids;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::summonable_entities;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::templates_under;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::chat_common::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::command::Registries;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::world_clock::OVERWORLD_CLOCK;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::world_clock::TimeMarker;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::{
    heightmap::{HeightmapState, HeightmapType},
    registry::{BlockRegistry, BlockStateId},
    worldgen::terrain::TerrainBlockStates,
};

impl Game<'_, crate::game::server_side::ServerState> {
    /// `IntegratedServer.setWorldAllowCommands`, then `updatePermissions`.
    pub(in crate::game) fn set_allow_commands(&mut self, allow: bool) {
        self.level.allow_commands = Some(allow);
        self.ui.registries.permission = owner_permission(allow);
    }
}
