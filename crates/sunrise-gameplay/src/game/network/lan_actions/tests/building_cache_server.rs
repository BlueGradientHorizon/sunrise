use crate::game::server_side::lan_actions_server::tick::building_cache_tests::Boxes;
use {crate::game::*, sunrise_gameplay_common::game::{Dim, lan_actions}};

/// The other players' boxes in the Builder's turn: what blocks its placing.
pub fn seen_by_builder(game: &mut crate::game::ServerGame<'_>) -> Boxes {
    let seen = game.as_guest(3, |g| g.server.lan_building_boxes.0.boxes(Dim::Overworld));

    seen.unwrap()
}
