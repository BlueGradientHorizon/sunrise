//! `ScaffoldingBlockItem.updatePlacementContext` reads the click's inside flag:
//! sneaking while the ray starts inside a scaffolding block places on the
//! opposite face, which the client's own raycast (`VoxelShape.clip`) supplies
//! and `BlockHitResult.isInside` carries.
#[allow(unused_imports)]
use crate::game::{Game, Hit, game_tests::tests::server_game};
#[allow(unused_imports)]
use sunrise_client::{command::args::GameMode, items::ItemStack, motion::PlayerMotion};
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
use winit::keyboard::KeyCode;

/// The block name at `pos`.
pub(in crate::game) fn name(game: &Game<'_>, pos: [i32; 3]) -> String {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    game.res.facts().get(state).name.to_string()
}
