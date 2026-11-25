//! What guests do in a hosted world. The host's gameplay code is written for
//! the one local player, so a guest's action runs through it with the guest's
//! state swapped in for the duration of the call (position, inventory, game
//! mode, experience, mining and item use): digging, placing, using items and
//! blocks, attacking and the commands all behave exactly as they do for the
//! host, and every world change they cause reaches all players through the
//! block log, the level events and the entity tracker.
use super::{
    Dim, Game, Mining,
    building_players::{BuildingPlayer, HubBodies, shared},
    lan::LanHost,
};
use mod_api::GameMode;
#[allow(unused_imports)]
use sunrise_client::{
    entities::Experience,
    first_person::{Hand, SwingAnimation},
    items::ItemStack,
    lan::{guest::SPECTATOR, menu::Slots},
    motion::PlayerMotion,
    pose::Dimensions,
};
use sunrise_core::geometry::Aabb;
#[allow(unused_imports)]
use sunrise_protocol::packets::play::{self, Clientbound as C, Serverbound as S};

pub(in crate::game) mod attack_checks;
pub(in crate::game) mod commands;
pub(in crate::game) mod death_state;
pub(in crate::game) mod effects;
pub(in crate::game) mod guest_body;
pub(in crate::game) mod hub_click;
pub(in crate::game) mod movement;
pub(in crate::game) mod packets;
pub(in crate::game) mod pickups;
pub(in crate::game) mod sleep;
pub(in crate::game) mod tick;
pub(in crate::game) mod turns;
#[allow(unused_imports)]
pub(in crate::game) use turns::{CombatPlayer, DisplacedPlayer};
include!("shared.rs");
