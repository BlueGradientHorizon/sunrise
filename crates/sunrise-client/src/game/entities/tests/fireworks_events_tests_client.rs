#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::events_tests::start_events;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::events_tests::stop_events;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::events_tests::take_explosions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::events_tests::twinkling;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::events_tests::with_game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fireworks::events_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{
    Audio, Options, Progress, Settings, load_resources,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    chunk::Chunk,
    items::ItemStack,
    mobs::Who,
    particles::{FireworkExplosion, FireworkShape, ParticleLevel},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::Vec3, position::ChunkPos, random::LegacyRandom,
};

#[test]
#[ignore = "daily"]
fn a_falling_rocket_hits_the_ground_and_one_in_water_splashes() {
    with_game(|game| {
        // Commands and collisions need resident terrain for both rocket columns.
        // Use the shared chunk fixture and the player's real ticket sources.
        crate::game::native_entity_refs::tests::prepare_entity_ticking_neighborhood(
            game,
            ChunkPos::new(0, 0),
        );
        game.build_registries();
        game.set_allow_commands(true);
        game.update_chunk_tickets();
        assert!(game.run_command("setblock 6 199 10 stone"));
        // Keep the falling rocket submerged through the sixth move: one
        // floating source block ends at y = 200, above its final position.
        assert!(game.run_command("setblock 20 199 10 water"));
        assert!(game.run_command("setblock 20 200 10 water"));
        for x in [6, 20] {
            assert!(game.run_command(&format!(
                "summon firework_rocket {x}.5 202 10.5 {{LifeTime:100,HasBeenShot:1b,ShotAtAngle:1b,Motion:[0d,-0.5d,0d]}}"
            )));
        }
        game.start_audio_capture();
        start_events();
        let mut falls = Vec::new();
        for _ in 0..6 {
            game.tick_rockets();
            falls.push(game.rockets[0].fall_distance);
        }
        let events: Vec<_> = crate::game::sculk::TEST_POSTED_EVENTS.with(|v| {
            std::mem::take(v.borrow_mut().as_mut().unwrap())
                .into_iter()
                .map(|e| (e.0, e.1[0].floor() as i32))
                .filter(|e| matches!(e.0, "minecraft:hit_ground" | "minecraft:splash"))
                .collect()
        });
        stop_events();
        // The fourth move ends on the stone without a collision; the fifth lands.
        assert_eq!(falls, [0.5, 1.0, 1.5, 2.0, 0.0, 0.0]);
        assert_eq!(game.rockets[1].fall_distance, 0.0);
        assert_eq!(
            events,
            [("minecraft:splash", 20), ("minecraft:hit_ground", 6)]
        );
        assert_eq!(
            game.server
                .pending_sounds
                .iter()
                .filter(|s| s.event == "minecraft:entity.generic.splash")
                .count(),
            1
        );
        game.stop_audio_capture();
    });
}

/// `FireworkParticles.Starter.playSound`: a silent rocket launches quietly (as
/// it already did) and bursts quietly too, drawing its sparks and flash only.
#[test]
#[ignore = "daily"]
fn a_silent_rocket_bursts_without_blast_or_twinkle() {
    with_game(|game| {
        let stack = ItemStack::new("minecraft:firework_rocket", 1);
        let mut launching = game.new_rocket(
            Vec3::new(6.5, 201.5, 10.5),
            &stack,
            false,
            &mut LegacyRandom::new(0),
        );
        launching.silent = true;
        launching.lifetime = 100;
        game.rockets.push(launching);
        game.start_audio_capture();
        game.tick_rockets();
        let heard = std::mem::take(&mut game.server.pending_sounds);
        game.stop_audio_capture();
        assert!(
            heard
                .iter()
                .all(|s| s.event != "minecraft:entity.firework_rocket.launch"),
            "a silent rocket launches quietly: {heard:?}"
        );
    });
    crate::game::native_entity_refs::tests::with_client_game(|game| {
        use sunrise_protocol::packets::play::{Clientbound as C, EntityEvent};
        game.player = Some(
            sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::new(2.5, 200.0, 2.5))
                .unwrap(),
        );
        let (quiet_at, loud_at) = ([6.5, 201.5, 10.5], [12.5, 201.5, 10.5]);
        let mut quiet = crate::game::fireworks::Rocket::remote(
            101,
            Vec3::new(quiet_at[0], quiet_at[1], quiet_at[2]),
            Vec3::ZERO,
        );
        quiet.silent = true;
        quiet.explosions = vec![twinkling()].into();
        let mut loud = crate::game::fireworks::Rocket::remote(
            102,
            Vec3::new(loud_at[0], loud_at[1], loud_at[2]),
            Vec3::ZERO,
        );
        loud.explosions = vec![twinkling()].into();
        game.rockets.push(quiet);
        game.rockets.push(loud);
        for id in [101, 102] {
            game.apply_packet(C::EntityEvent(EntityEvent {
                id,
                event: crate::game::fireworks::FIREWORK_EXPLODE_EVENT,
            }))
            .unwrap();
        }
        game.particles.0.sounds.clear();
        for _ in 0..20 {
            tick_particles(game);
        }
        let bursts: Vec<(&str, [f64; 3])> = game
            .particles
            .0
            .sounds
            .iter()
            .filter(|s| s.0.starts_with("minecraft:entity.firework_rocket."))
            .map(|s| (s.0, s.1))
            .collect();
        assert_eq!(
            bursts,
            vec![
                ("minecraft:entity.firework_rocket.blast", loud_at),
                ("minecraft:entity.firework_rocket.twinkle", loud_at),
            ],
            "only the loud rocket bangs and twinkles, both at its own position"
        );
        assert!(
            !game.particles.0.is_empty(),
            "the silent rocket still drew its sparks and flash"
        );
    });
}

pub(in crate::game::server_side) fn tick_particles(game: &mut Game<'_>) {
    let level = ParticleLevel {
        world: &game.world,
        facts: game.res.facts(),
        collision: game.collision,
        player: None,
        cached_chunk: std::cell::Cell::new(None),
    };
    game.particles.0.tick(&level);
}
