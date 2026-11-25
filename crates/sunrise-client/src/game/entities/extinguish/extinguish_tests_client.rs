#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::game_tests::tests::server_game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::extinguish::extinguish_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::motion::{MotionWorld, PlayerMotion};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::chunk::BlockState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::entity_move::MovementRecord;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::registry::BlockStateId;

pub(in crate::game) fn heard(game: &crate::game::ClientGame<'_>) -> u32 {
    game.side.audio
        .0
        .heard
        .get("minecraft:entity.generic.extinguish_fire")
        .copied()
        .unwrap_or(0)
}
