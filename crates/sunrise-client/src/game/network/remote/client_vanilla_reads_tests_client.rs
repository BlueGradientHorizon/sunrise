#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_vanilla_reads_tests::PLAYER;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_vanilla_reads_tests::add;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_vanilla_reads_tests::join;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_vanilla_reads_tests::spawn;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_vanilla_reads_tests::tick_count;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_vanilla_reads_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{
    Game, Screen, native_entity_refs::tests::with_client_game,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::ClientHost;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::chunk::Chunk;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::{
    connection::{ConnReader, ConnWriter},
    packets::play::{self, Clientbound as C},
};

/// The server's teleport that makes the local player, on a stone floor, with the level loaded.
pub(in crate::game) fn stand(game: &mut Game<'_>, teleport: i32) {
    game.streaming
        .insert_remote(&mut game.world, Chunk::empty(0, 0));
    let stone = game
        .pack
        .assets
        .registry
        .default_state("minecraft:stone")
        .unwrap();
    for x in 0..16 {
        for z in 0..16 {
            game.world.set(x, 63, z, stone).unwrap();
        }
    }
    game.handle_packet(C::PlayerPosition(play::PlayerPosition {
        teleport,
        position: [0.5, 64.0, 0.5],
        delta: [0.0; 3],
        yaw: 0.0,
        pitch: 0.0,
        relatives: 0,
    }))
    .unwrap();
    let remote = game.remote.as_mut().unwrap();
    remote.loaded_sent = true;
    remote.level_load.finish();
    game.client.screen = Screen::Playing;
}
