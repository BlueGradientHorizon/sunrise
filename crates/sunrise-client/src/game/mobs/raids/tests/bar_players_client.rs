#[allow(unused_imports)]
pub(in crate::game::client_side) use super::super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::raids::bar_players_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::chunk::Chunk;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{geometry::Vec3, position::ChunkPos};

/// The red, ten-notched bars on this client's overlay: the raid's.
pub(in crate::game) fn raid_bars(game: &crate::game::ClientGame<'_>) -> usize {
    game.side
        .boss_overlay
        .bars(sunrise_client::boss_overlay::now_millis())
        .into_iter()
        .filter(|b| b.color == 2 && b.overlay == 2)
        .count()
}
