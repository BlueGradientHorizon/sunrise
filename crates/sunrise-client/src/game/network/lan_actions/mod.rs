//! What guests do in a hosted world. The host's gameplay code is written for
//! the one local player, so a guest's action runs through it with the guest's
//! state swapped in for the duration of the call (position, inventory, game
//! mode, experience, mining and item use): digging, placing, using items and
//! blocks, attacking and the commands all behave exactly as they do for the
//! host, and every world change they cause reaches all players through the
//! block log, the level events and the entity tracker.
#[allow(unused_imports)]
use super::{
    Dim, Game, GuestUse, Hit, InteractTarget, Mining, Screen,
    building_players::{BuildingPlayer, HubBodies, shared},
    lan::LanHost,
    lan_crafting::MenuRemoval,
    level_sounds::LevelSound,
    respawn_anchor::RespawnConfig,
};
#[allow(unused_imports)]
use mod_api::{ActionRefused, GameMode, SoundSource};
#[allow(unused_imports)]
use sunrise_client::{
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
use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::RandomSource,
};
#[allow(unused_imports)]
use sunrise_protocol::{
    packets::play::{self, Clientbound as C, Serverbound as S},
    text::literal,
};
#[allow(unused_imports)]
use sunrise_world::entity_move::EntityMoveWorld;
#[allow(unused_imports)]
use winit::keyboard::KeyCode;

pub(in crate::game) mod attack_checks;
pub(in crate::game) mod commands;
pub(in crate::game) mod death_state;
pub(in crate::game) mod effects;
pub(in crate::game) mod guest_body;
pub(in crate::game) mod hub_click;
pub(in crate::game) mod interaction;
pub(in crate::game) mod menus;
pub(in crate::game) mod mining;
pub(in crate::game) mod movement;
pub(in crate::game) mod packets;
pub(in crate::game) mod pickups;
pub(in crate::game) mod respawn;
pub(in crate::game) mod sleep;
pub(in crate::game) mod tick;
pub(in crate::game) mod turns;
#[allow(unused_imports)]
pub(in crate::game) use turns::{CombatPlayer, DisplacedPlayer};
include!("shared.rs");
