#[allow(unused_imports)]
pub(in crate::game::client_side) use super::super::{C, Game, play};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_emitters_tests::add_entity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_emitters_tests::animate;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_emitters_tests::deliver;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_emitters_tests::registry_id;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_emitters_tests::with_isolated_client;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::client_emitters_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{
    native_entity_refs::tests::with_client_game, native_hotbar_slot::hotbar_slot_tests::logged_in,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::particles::ParticleType::{
    self, Crit, EnchantedHit,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::{LegacyRandom, RandomSource};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::entity_data::{Value, serializer};

// Capture the level requests, before the limiter or providers consume random
// draws. The expected sequence uses the 26.3 TrackingEmitter sampling contract
// and independent feet/dimensions, never the packet handler's body lookup.
pub(in crate::game) fn assert_bursts(
    game: &mut Game<'_>,
    random: &mut LegacyRandom,
    bodies: &[(ParticleType, [f64; 3], f32, f32)],
) {
    let mut expected = Vec::new();
    for &(ty, [x, y, z], width, height) in bodies {
        for _ in 0..16 {
            let [dx, dy, dz] = std::array::from_fn(|_| f64::from(random.next_float()) * 2.0 - 1.0);
            if dx * dx + dy * dy + dz * dz <= 1.0 {
                let at = [
                    x + f64::from(width) * (0.25 * dx),
                    y + f64::from(height) * (0.5 + 0.25 * dy),
                    z + f64::from(width) * (0.25 * dz),
                ];
                expected.push((
                    ty,
                    at.map(f64::to_bits),
                    [dx, dy + 0.2, dz].map(f64::to_bits),
                ));
            }
        }
    }
    let actual: Vec<_> = game
        .particles
        .0
        .requested
        .as_mut()
        .unwrap()
        .drain(..)
        .map(|(options, at, motion)| (options.ty, at.map(f64::to_bits), motion.map(f64::to_bits)))
        .collect();
    assert_eq!(actual, expected);
}

pub(in crate::game) fn capture_requests(game: &mut Game<'_>) -> LegacyRandom {
    const SEED: i64 = 719;
    game.particles.0.seed_randoms(SEED, 11, 12);
    game.particles.0.requested = Some(Vec::new());
    LegacyRandom::new(SEED)
}
