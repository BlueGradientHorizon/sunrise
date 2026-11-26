use super::*;

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
    game.side
        .streaming
        .insert_remote(&mut game.world, sunrise_level::chunk::Chunk::empty(0, 0));
    server
}
