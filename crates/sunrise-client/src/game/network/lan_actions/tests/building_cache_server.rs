use crate::game::lan_actions::tick::building_cache_tests::Boxes;
use crate::game::*;

/// The other players' boxes in the Builder's turn: what blocks its placing.
pub(in crate::game) fn seen_by_builder(game: &mut crate::game::ServerGame<'_>) -> Boxes {
    let seen = game.as_guest(3, |g| g.side.lan_building_boxes.0.boxes(Dim::Overworld));

    seen.unwrap()
}
