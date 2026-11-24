#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::tick::building_cache_tests::Boxes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::tick::building_cache_tests::game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::tick::building_cache_tests::mover_at;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::tick::building_cache_tests::the_mover_changed_mid_tick;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::tick::building_cache_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{Arc, atomic::AtomicI32, mpsc};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    chunk::Chunk,
    lan::{Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig},
    motion::PlayerMotion,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::configuration::ClientInformation;

/// The other players' boxes in the Builder's turn: what blocks its placing.
pub(in crate::game) fn seen_by_builder(game: &mut Game<'_>) -> Boxes {
    let seen = game.as_guest(3, |g| g.lan_building_boxes.0.boxes(Dim::Overworld));

    seen.unwrap()
}
