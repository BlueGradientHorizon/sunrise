#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Dim, Game, GuestUse, Hit, InteractTarget, Mining, Screen,
    building_players::{BuildingPlayer, HubBodies, shared},
    lan::LanHost,
    lan_crafting::MenuRemoval,
    level_sounds::LevelSound,
    respawn_anchor::RespawnConfig,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::BUILDING_CACHE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::GuestCommand;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::GuestPlay;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::ItemRules;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::MenuOpen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::RECIPE_CHECK_COST;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::RECIPE_CHECK_LIMIT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::StopDig;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::TurnRefused;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::building_cache;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::building_player;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::building_players;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::bundle_inventory_slot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::equipment_changed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::eye_columns;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::first_spin_target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::guest_within_entity_range;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::refresh_building_player;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::reset_use_attack_strength;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::stop_dig;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::swing_packet;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::touching_unloaded_chunk;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::turns::{
    CombatPlayer, DisplacedPlayer,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::visible_aim_point;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{ActionRefused, GameMode, SoundSource};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    containers::BlockEntity,
    entities::Experience,
    first_person::{Hand, SwingAnimation},
    items::ItemStack,
    lan::{
        guest::SPECTATOR,
        hub::TickCtx,
        menu::{Rules, Slots},
    },
    motion::{MotionEvent, MotionWorld, PlayerMotion},
    pose::Dimensions,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::{
    packets::play::{self, Clientbound as C, Serverbound as S},
    text::literal,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::entity_move::EntityMoveWorld;
#[allow(unused_imports)]
pub(in crate::game::client_side) use winit::keyboard::KeyCode;

#[path = "interaction_client.rs"]
pub(in crate::game) mod interaction;

#[path = "pickups_client.rs"]
pub(in crate::game) mod pickups;
