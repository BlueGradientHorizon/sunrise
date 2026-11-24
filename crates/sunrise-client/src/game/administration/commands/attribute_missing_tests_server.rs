#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::world_cmds::attribute_missing_tests::game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::world_cmds::attribute_missing_tests::only;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::world_cmds::attribute_missing_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::motion::PlayerMotion;

/// Runs `command`: whether it succeeded and what the issuer was told.
pub(in crate::game) fn run(game: &mut Game<'_>, command: &str) -> (bool, Vec<String>) {
    game.server.chat_capture = Some(Vec::new());
    let ok = game.run_command(command);
    let said = game.server.chat_capture.take().unwrap_or_default();
    (
        ok,
        said.iter().map(sunrise_client::text::Text::plain).collect(),
    )
}
