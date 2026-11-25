#[allow(unused_imports)]
pub(in crate::game::server_side) use super::super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::clone_writes_tests::cloned;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::clone_writes_tests::game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::clone_writes_tests::load_chunk;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::clone_writes_tests::load_chunk_in;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::clone_writes_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::chunk::Chunk;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::position::ChunkPos;

/// Runs `command`: whether it succeeded and what it said first.
pub(in crate::game) fn run(
    game: &mut crate::game::ServerGame<'_>,
    command: &str,
) -> (bool, Option<String>) {
    game.side.chat_capture = Some(Vec::new());
    let ok = game.run_command(command);
    let said = game.side.chat_capture.take().unwrap_or_default();
    (ok, said.first().map(sunrise_client::text::Text::plain))
}
