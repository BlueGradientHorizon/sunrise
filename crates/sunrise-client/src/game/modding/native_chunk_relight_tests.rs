//! `level.relight_chunks` (queued, not lit synchronously) and `chunk.resend` through the public
//! handles, covering active and parked server levels as well as each named refusal.

use super::native_entity_refs::tests::{with_client_game, with_game};
use super::native_guest_turn_tests::with_guests;
use mod_api::{ChunkRefusal, LevelRef, LightLayer, WorldEpoch};
use std::sync::Arc;
use sunrise_client::chunk::Chunk;

/// A stone roof over the chunk (100, 100), all eight neighbours loaded around it.
fn roofed(game: &mut super::Game<'_>) {
    let stone = game
        .pack
        .assets
        .registry
        .default_state("minecraft:stone")
        .unwrap();
    for dz in -1..=1 {
        for dx in -1..=1 {
            let mut chunk = Chunk::empty(100 + dx, 100 + dz);
            if dx == 0 && dz == 0 {
                for (x, z) in (0..16).flat_map(|x| (0..16).map(move |z| (x, z))) {
                    chunk.set(x, 100, z, stone.0);
                }
            }
            game.world
                .chunks
                .insert((100 + dx, 100 + dz), Arc::new(chunk));
        }
    }
}

fn stale(level: LevelRef) -> LevelRef {
    LevelRef::new(WorldEpoch::new(level.epoch().get() + 1), level.dimension())
}

/// A relight is queued at once, nothing is lit during the call, and the count is the columns
/// queued (the loaded ring included); a chunk not itself loaded queues none (not even loaded
/// neighbours), and large batches are accepted without artificial per-call or queue caps.
#[test]
#[ignore = "daily"]
fn a_mod_queues_the_chunks_it_wrote() {
    with_game(|game| {
        roofed(game);
        let level = game.level(game.dim.id()).unwrap();
        game.world.dirty_light.clear();
        // Unloaded chunk adjacent to a loaded chunk queues nothing.
        assert_eq!(level.relight_chunks(game, &[[102, 100]]), Ok(0));
        assert!(game.world.dirty_light.is_empty());
        assert_eq!(level.relight_chunks(game, &[[100, 100]]), Ok(9));
        assert_eq!(game.world.dirty_light.len(), 9);
        assert!(game.world.dirty_light.contains(&(100, 100)));
        // Asked again before the light update: nothing new is queued.
        assert_eq!(level.relight_chunks(game, &[[100, 100]]), Ok(0));
        assert_eq!(level.relight_chunks(game, &[[400, 400]]), Ok(0));
        assert_eq!(game.world.dirty_light.len(), 9);
    });
}

/// Calls past the former 256-chunk and 4096-queued limits are accepted and scheduled on the
/// owning level's queue without refusal.
#[test]
#[ignore = "daily"]
fn relight_chunks_accepts_large_batches_without_synthetic_quotas() {
    with_game(|game| {
        roofed(game);
        let level = game.level(game.dim.id()).unwrap();
        game.world.dirty_light.clear();
        game.world
            .dirty_light
            .extend((0..4200).map(|i| (1000 + i, 1000)));
        let mut chunks: Vec<[i32; 2]> = (0..300).map(|i| [400 + i, 400]).collect();
        chunks.push([100, 100]);
        assert_eq!(level.relight_chunks(game, &chunks), Ok(9));
        assert_eq!(game.world.dirty_light.len(), 4209);
        assert!(game.world.dirty_light.contains(&(100, 100)));
    });
}

/// A handle of an earlier open of the world is `Stale`, for the level and for a chunk of it.
#[test]
#[ignore = "daily"]
fn a_stale_handle_is_refused_by_name() {
    with_game(|game| {
        roofed(game);
        let level = game.level(game.dim.id()).unwrap();
        assert_eq!(
            stale(level).relight_chunks(game, &[[100, 100]]),
            Err(ChunkRefusal::Stale)
        );
        let chunk = mod_api::__private::game::chunk_ref(stale(level), [100, 100]);
        assert_eq!(chunk.resend(game), Err(ChunkRefusal::Stale));
    });
}

/// A parked open level accepts `relight_chunks` and `chunk.resend` on its own `World` without
/// touching the level in place, advances its queued light jobs on `update_parked_light`, and
/// refuses unloaded chunks with `NotLoaded`.
#[test]
#[ignore = "daily"]
fn parked_level_relight_and_resend_target_the_owning_dimension() {
    with_game(|game| {
        roofed(game);
        let overworld = game.level("minecraft:overworld").unwrap();
        game.world.block_log = Some(Vec::new());
        game.world.dirty_light.clear();
        game.world.bulk_log.clear();
        game.enter_level(super::Dim::Nether).unwrap();
        // Neither Nether's dirty_light nor bulk_log should be touched by Overworld calls.
        game.world.dirty_light.clear();
        game.world.bulk_log.clear();

        assert_eq!(overworld.relight_chunks(game, &[[100, 100]]), Ok(9));
        assert!(game.world.dirty_light.is_empty());
        assert_eq!(
            game.levels
                .parked_world_of("minecraft:overworld")
                .unwrap()
                .dirty_light
                .len(),
            9
        );

        let loaded = mod_api::__private::game::chunk_ref(overworld, [100, 100]);
        assert_eq!(loaded.resend(game), Ok(()));
        assert!(game.world.bulk_log.is_empty());
        assert!(
            game.levels
                .parked_world_of("minecraft:overworld")
                .unwrap()
                .bulk_log
                .contains(&(100, 100))
        );
        let unloaded = mod_api::__private::game::chunk_ref(overworld, [400, 400]);
        assert_eq!(unloaded.resend(game), Err(ChunkRefusal::NotLoaded));

        // Drive parked light updates until the queued roofed column finishes lighting.
        // `with_game` creates `Game` on the title screen with `streaming.paused = true`;
        // unpause the parked Overworld streaming pool as an open world does.
        game.levels
            .parked_mut(super::Dim::Overworld)
            .unwrap()
            .streaming
            .paused = false;
        for _ in 0..40 {
            game.update_parked_light(&[super::Dim::Nether]);
            if !game
                .levels
                .parked
                .get(&super::Dim::Overworld)
                .unwrap()
                .streaming
                .has_pending_light(
                    &game
                        .levels
                        .parked
                        .get(&super::Dim::Overworld)
                        .unwrap()
                        .world,
                )
            {
                break;
            }
            game.levels
                .parked_mut(super::Dim::Overworld)
                .unwrap()
                .streaming
                .wait(std::time::Duration::from_millis(50));
        }
        // Under the stone roof at y = 100, sky light at (1608, 99, 1608) is no longer 15.
        let sky_under_roof = game.level_brightness(overworld, LightLayer::Sky, [1608, 99, 1608]);
        assert!(
            sky_under_roof.is_some_and(|v| v < 15),
            "expected parked relight to darken sky light beneath the roof, got {sky_under_roof:?}"
        );
    });
}

/// On a parked level without an active `block_log`, `chunk.resend` checks real sent-chunk
/// watchers on that dimension before queueing `bulk_log`.
#[test]
#[ignore = "daily"]
fn parked_chunk_resend_checks_sent_watchers_on_owning_dimension() {
    with_guests(|game| {
        roofed(game);
        let overworld = game.level("minecraft:overworld").unwrap();
        game.world.block_log = None;
        game.world.bulk_log.clear();
        game.enter_level(super::Dim::Nether).unwrap();

        let chunk = mod_api::__private::game::chunk_ref(overworld, [100, 100]);
        // No guest has been sent (100, 100) in Overworld yet: Ok, nothing queued.
        assert_eq!(chunk.resend(game), Ok(()));
        assert!(
            !game
                .levels
                .parked_world_of("minecraft:overworld")
                .unwrap()
                .bulk_log
                .contains(&(100, 100))
        );

        // Once a guest watching Overworld has been sent (100, 100), resend queues it.
        game.lan
            .as_mut()
            .unwrap()
            .hub
            .guests
            .update(0, |g| _ = g.sent_chunks.insert((100, 100)));
        assert_eq!(chunk.resend(game), Ok(()));
        assert!(
            game.levels
                .parked_world_of("minecraft:overworld")
                .unwrap()
                .bulk_log
                .contains(&(100, 100))
        );
    });
}

/// A chunk that is not loaded is `NotLoaded`; a loaded one is queued to go out whole.
#[test]
#[ignore = "daily"]
fn a_chunk_is_resent_only_when_loaded() {
    with_game(|game| {
        roofed(game);
        let level = game.level(game.dim.id()).unwrap();
        game.world.block_log = Some(Vec::new());
        game.world.bulk_log.clear();
        let loaded = game.chunk(level, [100, 100]).unwrap();
        assert_eq!(loaded.resend(game), Ok(()));
        assert!(game.world.bulk_log.contains(&(100, 100)));
        let unloaded = mod_api::__private::game::chunk_ref(level, [400, 400]);
        assert_eq!(unloaded.resend(game), Err(ChunkRefusal::NotLoaded));
        assert!(!game.world.bulk_log.contains(&(400, 400)));
    });
}

/// On a client both are `Client`.
#[test]
#[ignore = "daily"]
fn a_client_cannot_relight_or_resend() {
    with_client_game(|game| {
        roofed(game);
        let level = LevelRef::new(WorldEpoch::new(0), game.dim.key());
        assert_eq!(
            level.relight_chunks(game, &[[100, 100]]),
            Err(ChunkRefusal::Client)
        );
        let chunk = mod_api::__private::game::chunk_ref(level, [100, 100]);
        assert_eq!(chunk.resend(game), Err(ChunkRefusal::Client));
    });
}
