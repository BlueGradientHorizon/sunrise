#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::misc::random_command_tests::chats;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::misc::random_command_tests::packets;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::misc::random_command_tests::server_with_guests;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::misc::random_command_tests::suggestions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::misc::random_command_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{Dim, game_tests::tests::server_game};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{Arc, atomic::AtomicI32, mpsc};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::lan::{
    Joined, NetConfig, Out, PlayerSender, guest::Guest, hub::HubConfig,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::{
    configuration::ClientInformation, play::Clientbound,
};

/// CommandSourceStack.getDisplayName: a command block names itself, the
/// console says "Server".
#[test]
#[ignore = "daily"]
fn a_blocks_roll_names_the_block_and_the_console_says_server() {
    let mut resources = None;
    let mut game = server_game(&mut resources);
    game.build_registries();
    game.set_allow_commands(true);
    assert!(game.run_command(
        r#"setblock 0 200 0 minecraft:command_block{Command:"random roll 1..2",CustomName:{text:"Dice"}}"#
    ));
    let roll = |game: &mut crate::game::ServerGame<'_>, source: crate::game::commands::execute::Source| {
        let outer = game.side.command_source.replace(source);
        game.side.chat_capture = Some(Vec::new());
        assert!(game.run_command("random roll 1..2"));
        game.side.command_source = outer;
        let lines = game.side.chat_capture.take().unwrap();
        assert_eq!(lines.len(), 1, "{:?}", lines[0].plain());
        lines[0].plain()
    };
    let block = crate::game::commands::execute::Source {
        entity: None,
        block: Some((0, 200, 0)),
        ..game.server_source()
    };
    let named = roll(&mut game, block);
    assert!(named.starts_with("Dice rolled "), "{named}");
    assert!(named.ends_with(" (from 1 to 2)"), "{named}");
    let console = game.server_source();
    let named = roll(&mut game, console);
    assert!(named.starts_with("Server rolled "), "{named}");
    assert!(named.ends_with(" (from 1 to 2)"), "{named}");
}
