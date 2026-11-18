#![cfg(test)]
//! `animate_tick`: the picks of `ClientLevel.animateTick`'s sampling whose block has a part
//! answering it reach that part, in pick order, with the level's random; other blocks' picks are
//! never kept, and with no part answering nothing is recorded at all.
use crate::game::{
    native_client_session::tests::logged_client, native_entity_refs::tests::with_client_game,
};
use mod_api::{
    AnimateTickEvent, BlockPart, BlockPos, ClientHost, Hooks, ModEntry, ParticleOptions, Registrar,
    RegistrationContext,
};
use std::sync::{Arc, Mutex};
use sunrise_client::{chunk::Chunk, motion::PlayerMotion};
use sunrise_core::{
    geometry::Vec3,
    random::{LegacyRandom, RandomSource as _},
};

/// What the part heard: each call's position, block and first draw, and whether its spark was
/// submitted.
type Calls = Arc<Mutex<Vec<(BlockPos, String, f32, bool)>>>;

struct Sparks {
    calls: Calls,
    spark: mod_api::ParticleKey,
}
#[mod_api::sunrise_part]
impl BlockPart for Sparks {
    fn animate_tick(&self, host: &mut dyn ClientHost, event: &mut AnimateTickEvent<'_>) {
        let draw = event.random.next_float();
        let p = event.pos;
        let at = [
            f64::from(p.x) + 0.5,
            f64::from(p.y) + 0.5,
            f64::from(p.z) + 0.5,
        ];
        let added = host.add_particle(&ParticleOptions::simple(self.spark), at, [0.0; 3]);
        let block = event.state.block().id(&*host).to_string();
        self.calls.lock().unwrap().push((p, block, draw, added));
    }
}
struct Register(Calls);
#[mod_api::__subscriptions]
impl Hooks for Register {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let glass = r.block_key("minecraft:glass");
        let spark = r.particle_key("minecraft:electric_spark");
        r.block_part(
            glass,
            Sparks {
                calls: self.0.clone(),
                spark,
            },
        );
    }
}

#[test]
#[ignore = "daily"]
fn sampled_picks_reach_the_parts_of_their_block_only() {
    for installed in [false, true] {
        let calls = Calls::default();
        let mods = if installed {
            vec![ModEntry::new("sparks", Box::new(Register(calls.clone())))]
        } else {
            Vec::new()
        };
        mod_platform::testing::with_mods(mods.into(), || {
            with_client_game(|game| {
                let _server = logged_client(game);
                for cx in -2..=2 {
                    for cz in -2..=2 {
                        game.world
                            .chunks
                            .insert((cx, cz), Arc::new(Chunk::empty(cx, cz)));
                    }
                }
                game.player =
                    Some(PlayerMotion::new(game.motion, Vec3::new(8.5, 65.0, 8.5)).unwrap());
                let registry = &game.pack.assets.registry;
                let glass = registry.default_state("minecraft:glass").unwrap();
                let stone = registry.default_state("minecraft:stone").unwrap();
                if !installed {
                    assert!(
                        game.mod_animate_picks().is_none(),
                        "with no part answering, the sampler records nothing"
                    );
                    return;
                }
                // Recorded picks: the glass ones are kept and called in order, the stone one
                // (no part) is dropped; each draw moves the level's random.
                let mut expected =
                    LegacyRandom::from(<(u64, Option<f64>)>::from(&*game.particles.level_random()));
                let picks = game.mod_animate_picks().unwrap();
                picks.record(glass, [1, 64, 2]);
                picks.record(stone, [3, 64, 4]);
                picks.record(glass, [5, 64, 6]);
                game.mod_animate_ticks(picks);
                let first = expected.next_float();
                let second = expected.next_float();
                let glass_id = "minecraft:glass".to_owned();
                assert_eq!(
                    *calls.lock().unwrap(),
                    [
                        (BlockPos::new(1, 64, 2), glass_id.clone(), first, true),
                        (BlockPos::new(5, 64, 6), glass_id.clone(), second, true),
                    ]
                );
                assert_eq!(
                    game.particles.level_random().next_long(),
                    expected.next_long(),
                    "the parts drew the level's own random"
                );
                // A client tick's real sampling: a floor of glass under the player with a stone
                // row; only glass positions reach the part.
                calls.lock().unwrap().clear();
                for x in -8..24 {
                    for z in -8..24 {
                        let state = if x == 8 { stone } else { glass };
                        game.world.set(x, 64, z, state).unwrap();
                    }
                }
                for _ in 0..4 {
                    game.tick_particles();
                }
                let calls = calls.lock().unwrap();
                assert!(!calls.is_empty(), "the sampling picked a glass block");
                for (pos, block, _, _) in calls.iter() {
                    assert_eq!(pos.y, 64);
                    assert_ne!(pos.x, 8, "the stone row has no part");
                    assert_eq!(*block, glass_id);
                }
            })
        });
    }
}
