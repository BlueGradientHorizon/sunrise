#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::{LegacyRandom, RandomSource};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::level_particles::phantom_bite_tests::*;

pub(in crate::game) fn with_game(test: impl FnOnce(&mut Game<'_>)) {
    let mut resources = None;
    let mut g = crate::game::game_tests::tests::server_game(&mut resources);
    g.options.dedicated = Some(Vec::new());
    g.world = World::new(g.res.facts().air, 1);
    // The guests' copy of a level event is recorded only while capturing.
    g.audio.0.capture = Some(Vec::new());
    test(&mut g);
}
