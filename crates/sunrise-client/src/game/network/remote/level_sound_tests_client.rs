#[allow(unused_imports)]
pub(in crate::game::client_side) use super::super::native_client_session::tests::logged_client;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::super::native_entity_refs::tests::with_client_game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::level_sound_tests::LocalTuner;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::level_sound_tests::Log;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::level_sound_tests::Request;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::level_sound_tests::Tuner;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::level_sound_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{Flow, HostSide, SoundPlay, SoundSource};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Mutex;

#[test]
#[ignore = "daily"]
fn game_events_ask_client_level_before_playback() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mods = vec![mod_api::ModEntry::new(
        "game_event_tuner",
        Box::new(LocalTuner(requests.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_client_game(|game| {
            let _server = logged_client(game);
            let mut player = PlayerMotion::new(game.motion, Vec3::new(1.0, 64.0, 1.0)).unwrap();
            player.dimensions.eye_height = 1.25;
            game.player = Some(player);
            for (event, param) in [(6, 1.0), (9, 1.0), (10, 1.4)] {
                game.handle_packet(C::GameEvent(play::GameEvent { event, param }))
                    .unwrap();
            }
            assert_eq!(
                *requests.lock().unwrap(),
                [
                    Request {
                        sound: "minecraft:entity.arrow.hit_player".into(),
                        source: SoundSource::Players,
                        emitter: None,
                        position: Some([1.0, 65.25, 1.0])
                    },
                    Request {
                        sound: "minecraft:entity.puffer_fish.sting".into(),
                        source: SoundSource::Neutral,
                        emitter: None,
                        position: Some([1.0, 64.0, 1.0])
                    },
                    Request {
                        sound: "minecraft:entity.elder_guardian.curse".into(),
                        source: SoundSource::Hostile,
                        emitter: None,
                        position: Some([1.0, 64.0, 1.0])
                    },
                ]
            );
            assert!(
                game.side.audio.0.heard.is_empty(),
                "all three canceled requests stop before Audio"
            );
            game.handle_packet(C::GameEvent(play::GameEvent {
                event: 10,
                param: 1.5,
            }))
            .unwrap();
            assert_eq!(
                requests.lock().unwrap().len(),
                3,
                "a silent curse asks no sound hook"
            );
            let level = sunrise_client::particles::ParticleLevel {
                world: &game.world,
                facts: game.res.facts(),
                collision: game.collision,
                player: None,
                cached_chunk: std::cell::Cell::new(None),
            };
            game.side.particles.0.tick(&level);
            assert_eq!(
                game.side.particles.0.elder_guardians().len(),
                2,
                "neither silent nor canceled sound suppresses the apparition"
            );
        });
    });
}

#[test]
#[ignore = "daily"]
fn local_level_requests_can_mute_sound_without_undoing_prediction() {
    use sunrise_core::position::BlockPos;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mods = vec![mod_api::ModEntry::new(
        "local_tuner",
        Box::new(LocalTuner(requests.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_client_game(|game| {
            let _server = logged_client(game);
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(1.0, 64.0, 1.0)).unwrap());
            game.streaming
                .insert_remote(&mut game.world, Chunk::empty(0, 0));
            let old = game
                .pack
                .assets
                .registry
                .default_state("minecraft:oak_door")
                .unwrap();
            let new = game
                .pack
                .assets
                .registry
                .default_state("minecraft:birch_door")
                .unwrap();
            game.world.set(1, 64, 1, old).unwrap();
            let at = BlockPos::new(1, 64, 1);
            game.predict_use(
                &(
                    vec![(at, new)],
                    vec![(
                        at,
                        "block.wooden_door.open".into(),
                        mod_api::SoundSource::Blocks,
                        1.0,
                        0.9,
                    )],
                ),
                1,
            );
            assert_eq!(
                game.world.block(1, 64, 1),
                new,
                "sound cancellation does not cancel the use"
            );
            game.player_menu_sound(sunrise_client::bundle::INSERT_FAIL);
            let stack = ItemStack {
                item: "minecraft:apple".into(),
                count: 1,
                damage: 0,
                extra: None,
            };
            let consumable = game
                .items()
                .get(&stack.item)
                .unwrap()
                .consumable
                .clone()
                .unwrap();
            game.emit_consume(&consumable, &stack, 16);
            game.play_player_bound_sound("minecraft:item.goat_horn.sound.0", 2, 8.0, 1.0, true);
            assert!(
                game.side.audio.0.heard.is_empty(),
                "all four canceled requests stop before Audio"
            );
            let expected = [
                Request {
                    sound: "minecraft:block.wooden_door.open".into(),
                    source: SoundSource::Blocks,
                    emitter: None,
                    position: Some([1.5, 64.5, 1.5]),
                },
                Request {
                    sound: sunrise_client::bundle::INSERT_FAIL.into(),
                    source: SoundSource::Players,
                    emitter: Some(7),
                    position: None,
                },
                Request {
                    sound: "minecraft:entity.generic.eat".into(),
                    source: SoundSource::Players,
                    emitter: Some(7),
                    position: None,
                },
                Request {
                    sound: "minecraft:item.goat_horn.sound.0".into(),
                    source: SoundSource::Records,
                    emitter: Some(7),
                    position: None,
                },
            ];
            assert_eq!(*requests.lock().unwrap(), expected);
            game.side.audio.0.engine.loaded = true;
            game.play_client_player_sound("minecraft:block.honey_block.slide", 1.0, 1.0);
            assert_eq!(
                game.side.audio.0.heard.get("minecraft:block.honey_block.slide"),
                Some(&1)
            );
            assert!(
                game.side.audio.0.engine.bound_entities().is_empty(),
                "LocalPlayer.playSound stays positional"
            );
        });
    });
}
