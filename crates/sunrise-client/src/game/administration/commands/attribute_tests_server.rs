#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::world_cmds::attribute_tests::saved_attributes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::world_cmds::attribute_tests::with_game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::world_cmds::attribute_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::motion::PlayerMotion;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::Tag;

/// What a command says, and whether it succeeded.
pub(in crate::game) fn say(
    game: &mut crate::game::ServerGame<'_>,
    command: &str,
) -> (bool, String) {
    game.side.chat_capture = Some(Vec::new());
    let ok = game.run_command(command);
    let said = game.side.chat_capture.take().unwrap_or_default();
    (
        ok,
        said.iter().map(sunrise_client::text::Text::plain).collect(),
    )
}
