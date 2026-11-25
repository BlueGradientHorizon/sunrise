#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_entity_refs::tests::with_client_game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::movement_input_tests::Native;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::movement_input_tests::reset_player;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::movement_input_tests::tick;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::movement_input_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{ClientHost, Hooks, MovementInputUpdateEvent};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::motion::PlayerMotion;
#[allow(unused_imports)]
pub(in crate::game::client_side) use winit::keyboard::KeyCode;

#[path = "local_rotation_tests_client.rs"]
pub(in crate::game) mod local_rotation_tests;

pub(in crate::game) fn connect(
    game: &mut crate::game::ClientGame<'_>,
) -> sunrise_protocol::connection::Pipe {
    let (client, server) = sunrise_protocol::connection::Pipe::pair();
    game.connect_local(client, "movement-client").unwrap();
    let remote = game.remote.as_mut().unwrap();
    remote.logged_in = true;
    remote.loaded_sent = true;
    remote.entity_id = 42;
    remote.level_load.finish();
    game.side.screen = Screen::Playing;
    game.streaming
        .insert_remote(&mut game.world, sunrise_client::chunk::Chunk::empty(0, 0));
    server
}
