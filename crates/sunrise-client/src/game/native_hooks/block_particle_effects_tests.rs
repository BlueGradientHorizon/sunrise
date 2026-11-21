#![cfg(test)]
use crate::game::{
    Game, native_client_session::tests::logged_client, native_entity_refs::tests::with_client_game,
};
use mod_api::{
    AddDestroyEffectsEvent, AddHitEffectsEvent, BlockPart, BlockPos, BlockShapeKind, ClientHost,
    Direction, Flow, Hooks, ModEntry, Registrar, RegistrationContext,
};
use std::sync::{Arc, Mutex};
use sunrise_client::{chunk::Chunk, motion::PlayerMotion, particles::ParticleLevel};
use sunrise_core::{
    geometry::Vec3,
    random::{LegacyRandom, RandomSource as _},
};

type Calls = Arc<Mutex<Vec<(String, BlockPos, Option<Direction>)>>>;
struct Effects(Calls);
#[mod_api::sunrise_part]
impl BlockPart for Effects {
    fn add_hit_effects(
        &self,
        host: &mut dyn ClientHost,
        event: &mut AddHitEffectsEvent<'_>,
    ) -> Flow {
        let id = event.state.block().id(&*host).to_string();
        self.0
            .lock()
            .unwrap()
            .push((id.clone(), event.pos, Some(event.face)));
        if id != "minecraft:glass" {
            return Flow::Continue;
        }
        assert_eq!(
            event
                .level
                .block_shape(&*host, event.state, [8, 64, 4], BlockShapeKind::Outline),
            Some(vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]])
        );
        event.random.next_int();
        assert_eq!(
            event
                .level
                .add_breaking_particles(host, [8, 64, 4], event.state, event.face),
            Ok(true)
        );
        Flow::Cancel
    }

    fn add_destroy_effects(
        &self,
        host: &mut dyn ClientHost,
        event: &mut AddDestroyEffectsEvent<'_>,
    ) -> Flow {
        let id = event.state.block().id(&*host).to_string();
        self.0.lock().unwrap().push((id.clone(), event.pos, None));
        if id == "minecraft:glass" {
            assert_eq!(
                event.level.add_terrain_particle(
                    host,
                    event.state,
                    [4, 64, 4],
                    [8.5, 64.5, 4.5],
                    [0.0; 3]
                ),
                Ok(true)
            );
        }
        Flow::Cancel
    }
}

struct Register(Calls);
#[mod_api::__subscriptions]
impl Hooks for Register {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        for name in [
            "minecraft:glass",
            "minecraft:dirt",
            "minecraft:barrier",
            "minecraft:air",
        ] {
            let block = registrar.block_key(name);
            registrar.block_part(block, Effects(self.0.clone()));
        }
    }
}

fn with_effects(test: impl FnOnce(&mut Game<'_>, &Calls)) {
    let calls = Calls::default();
    let mods = vec![ModEntry::new("debris", Box::new(Register(calls.clone())))];
    mod_platform::testing::with_mods(mods.into(), || {
        with_client_game(|game| {
            let _server = logged_client(game);
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(4.5, 63.0, 4.5)).unwrap());
            game.audio.capture = Some(Vec::new());
            test(game, &calls);
        });
    });
}

fn admit(game: &mut Game<'_>) -> usize {
    game.particles.tick(&ParticleLevel {
        world: &game.world,
        facts: game.res.facts(),
        collision: game.collision,
        player: None,
        cached_chunk: std::cell::Cell::new(None),
    });
    game.particles.len()
}

#[test]
#[ignore = "daily"]
fn hit_effect_replaces_the_crumb_but_not_the_sound_or_level_random() {
    with_effects(|game, calls| {
        for (name, expected) in [
            ("glass", 1),
            ("dirt", 1),
            ("stone", 1),
            ("barrier", 0),
            ("air", 0),
        ] {
            let state = game
                .pack
                .assets
                .registry
                .default_state(&format!("minecraft:{name}"))
                .unwrap();
            game.world.set(4, 64, 4, state).unwrap();
            game.particles.clear();
            game.audio.heard.clear();
            game.particles.seed_randoms(10, 11, 12);
            game.breaking_block_effects([4, 64, 4], [-1, 0, 0], true);
            assert_eq!(admit(game), expected, "{name}");
            if name == "glass" {
                // The client-only hit sound intentionally bypasses the server's effect capture.
                assert_eq!(
                    game.audio.heard,
                    [("minecraft:block.glass.hit".into(), 1)].into()
                );
                let mut expected_random = LegacyRandom::new(11);
                expected_random.next_int();
                for _ in 0..3 {
                    expected_random.next_double();
                }
                assert_eq!(
                    game.particles.level_random().next_long(),
                    expected_random.next_long()
                );
                let mut quads = Vec::new();
                game.particles.render(
                    glam::DVec3::ZERO,
                    glam::Quat::IDENTITY,
                    0.0,
                    &|_| 0xff,
                    &mut quads,
                );
                assert_eq!(quads.len(), 4);
                assert!(quads.iter().all(|vertex| vertex.pos[0] > 7.0));
            } else if name == "air" {
                assert!(game.audio.heard.is_empty());
            }
        }
        let pos = BlockPos::new(4, 64, 4);
        assert_eq!(
            *calls.lock().unwrap(),
            [
                ("minecraft:glass".into(), pos, Some(Direction::West)),
                ("minecraft:dirt".into(), pos, Some(Direction::West))
            ]
        );
    });
}

#[test]
#[ignore = "daily"]
fn break_event_uses_the_destroyed_state_and_keeps_its_sound() {
    with_effects(|game, calls| {
        let glass = game
            .pack
            .assets
            .registry
            .default_state("minecraft:glass")
            .unwrap();
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        let barrier = game
            .pack
            .assets
            .registry
            .default_state("minecraft:barrier")
            .unwrap();
        let air = game
            .pack
            .assets
            .registry
            .default_state("minecraft:air")
            .unwrap();
        assert!(game.res.facts().get(stone).spawn_terrain_particles);
        assert!(!game.res.facts().get(barrier).spawn_terrain_particles);
        game.world.set(4, 64, 4, air).unwrap();
        game.level_event_at(2001, [4, 64, 4], glass.0 as i32);
        assert_eq!(admit(game), 1, "the part replaces the entire vanilla burst");
        assert_eq!(
            game.audio.capture.as_ref().unwrap()[0].event,
            "minecraft:block.glass.break"
        );
        assert_eq!(game.world.block(4, 64, 4), air);
        game.particles.clear();
        game.level_event_at(2001, [4, 64, 4], stone.0 as i32);
        assert_eq!(
            admit(game),
            64,
            "an unhandled full block gives the vanilla burst"
        );
        game.particles.clear();
        game.destroy_particles([4, 64, 4], barrier);
        assert_eq!(
            admit(game),
            0,
            "an invisible non-air state can handle destruction"
        );
        game.destroy_particles([4, 64, 4], air);
        assert_eq!(admit(game), 0);
        let pos = BlockPos::new(4, 64, 4);
        assert_eq!(
            *calls.lock().unwrap(),
            [
                ("minecraft:glass".into(), pos, None),
                ("minecraft:barrier".into(), pos, None)
            ]
        );
    });
}
