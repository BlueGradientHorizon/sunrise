//! Pufferfish.playerTouch in the real player's tick, independent of fish AI.
#[allow(unused_imports)]
use super::super::{
    Game,
    entity::{Entity, with_entity},
};
#[allow(unused_imports)]
use sunrise_client::{mobs::Species, scoreboard::teams_allow_damage};
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
use sunrise_protocol::packets::play::{Clientbound, GameEvent};

/// `ClientboundGameEventPacket.PUFFER_FISH_STING`.
pub(in crate::game) const PUFFER_FISH_STING: u8 = 9;

#[cfg(test)]
#[path = "pufferfish_sting_tests.rs"]
pub(in crate::game) mod pufferfish_sting_tests;
