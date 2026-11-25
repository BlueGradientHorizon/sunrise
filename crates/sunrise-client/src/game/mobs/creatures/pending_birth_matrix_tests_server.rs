use super::pending_birth_tests::{SEED, breed_pair, cube};
use super::*;

/// Two cows bred by the real route; the parent's id ends in `mode`.
pub(in crate::game) fn breed(
    game: &mut crate::game::ServerGame<'_>,
    mode: u32,
) -> (u32, u32, Option<u32>) {
    game.mobs.clear();
    game.side.next_entity = next_parent_id(game, mode) - 1;
    breed_pair(game, "minecraft:cow")
}

/// Select the fixture mode without reusing an ID issued to an earlier mob or breeding orb.
pub(in crate::game::server_side) fn next_parent_id(
    game: &crate::game::ServerGame<'_>,
    mode: u32,
) -> u32 {
    (game.side.next_entity + 1).next_multiple_of(10) + mode
}

/// A size-4 slime whose entity id ends in `mode`, split by the real route.
pub(in crate::game) fn split(game: &mut crate::game::ServerGame<'_>, mode: u32) -> Vec<Mob> {
    game.mobs.clear();
    let mut dying = cube(game, "minecraft:slime", 4, SEED);
    game.side.next_entity = next_parent_id(game, mode);
    dying.id = game.side.next_entity;
    game.split_cube_children(&mut dying);
    game.mobs.to_vec()
}
