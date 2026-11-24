#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::particle_command_tests::game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::particle_command_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{
    execution_role,
    launch_options::Options,
    resource_load::{Progress, load_resources},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::client::particles::PositionSource;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    audio::Audio,
    chunk::Chunk,
    motion::PlayerMotion,
    particles::{ParticleLevel, ParticleOptions, ParticleType},
    settings::Settings,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;

/// The crumbs the command names are drawn: `TerrainParticle.CrumblingProvider`
/// keeps no particle for options that carry no block.
#[test]
#[ignore = "daily"]
fn block_crumble_draws_the_block_it_names() {
    let mut resources = None;
    let mut game = game(&mut resources);
    game.particles.0.created_requests = None;
    game.particles.0.clear();
    assert!(game.run_command(
        "particle minecraft:block_crumble{block_state:\"minecraft:stone\"} 0.5 100.5 0.5 0 0 0 0 1"
    ));
    let level = ParticleLevel {
        world: &game.world,
        facts: game.res.facts(),
        collision: game.collision,
        player: None,
        cached_chunk: std::cell::Cell::new(None),
    };
    game.particles.0.tick(&level);
    assert_eq!(game.particles.0.len(), 1, "the crumble is drawn");
}

/// The one particle the window asks the engine for with `command`, at the
/// player's own block and with no spread.
pub(in crate::game) fn drawn(game: &mut Game<'_>, command: &str) -> ParticleOptions {
    game.particles.0.created_requests = Some(Vec::new());
    assert!(game.run_command(command), "{command}");
    let mut made = game.particles.0.created_requests.take().unwrap();
    assert_eq!(made.len(), 1, "{command}");
    made.remove(0).0
}
