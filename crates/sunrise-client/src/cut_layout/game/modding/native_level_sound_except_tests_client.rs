use sunrise_gameplay::game::native_guest_turn_tests::with_guests;
use sunrise_gameplay::game::server_side::native_level_sound_except_tests::decode_packets;
use sunrise_gameplay::game::server_side::native_level_sound_except_tests::in_isolated_process;
use mod_api::SoundSource;
use std::sync::{Arc, mpsc};
use sunrise_protocol::packets::play::Clientbound as C;
use sunrise_sim::lan::PlayerSender;

/// A transaction that emits Nether effects and commits while the Overworld is active replays each
/// effect in its recorded `Dim` (so Nether events/particles/sounds never leak to Overworld guests
/// and `PlayerRef::play_sound` advances Nether's `world.random` rather than the Overworld's) and
/// delivers packets on a single connection in exact call order across all effect kinds (including
/// `GlobalEvent(1038)` followed by `StopSound`, both outside and inside `as_guest`).
#[test]
#[ignore = "daily"]
fn cross_level_transaction_replays_effects_in_request_level_and_preserves_packet_order() {
    in_isolated_process(
        "cross_level_transaction_replays_effects_in_request_level_and_preserves_packet_order",
        || {
            use sunrise_core::random::RandomSource;
            use sunrise_game_data::gamerules::Value;
            use sunrise_level::particle_options::{ParticleOptions, ParticleType};

            with_guests(|game| {
                game.options.dedicated = Some(Vec::new());
                assert!(game.ui.rules.set("global_sound_events", Value::Bool(true)));
                let (tx12, rx12) = mpsc::channel();
                let (tx13, rx13) = mpsc::channel();
                game.server.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                    g.sender = PlayerSender::new(tx12, Arc::default());
                });
                game.server.lan.as_mut().unwrap().hub.guests.update(1, |g| {
                    g.sender = PlayerSender::new(tx13, Arc::default());
                    g.dimension = "minecraft:the_nether".to_owned();
                });
                // Load Nether once so it sits in `levels.parked` when Overworld is active.
                game.enter_level(sunrise_gameplay_common::game::Dim::Nether).unwrap();
                let mut expected_nether_random = game.world.random.clone();
                game.enter_level(sunrise_gameplay_common::game::Dim::Overworld).unwrap();
                let mut expected_overworld_random = game.world.random.clone();

                let player13 = game.player(13).unwrap();
                let entity13 = player13.entity();
                let entity13_id = i32::try_from(entity13.id()).unwrap();
                let at = [3.0, 64.0, 5.0];
                let pos = [3, 64, 5];
                let anvil = "minecraft:block.anvil.use";
                let bell = "minecraft:block.bell.use";
                let end_portal_spawn = "minecraft:block.end_portal.spawn";
                let anvil_id = game
                    .server
                    .lan
                    .as_ref()
                    .unwrap()
                    .hub
                    .registry_id("minecraft:sound_event", anvil)
                    .unwrap();
                let bell_id = game
                    .server
                    .lan
                    .as_ref()
                    .unwrap()
                    .hub
                    .registry_id("minecraft:sound_event", bell)
                    .unwrap();
                let heart = ParticleOptions::simple(ParticleType::Heart);
                decode_packets(&rx12);
                decode_packets(&rx13);

                // Open an outer transaction in Overworld, switch to Nether and open a Nether
                // transaction to issue Nether effects across all effect kinds, commit the Nether
                // transaction, and switch back to Overworld before closing the outer transaction.
                let committed = game.storage_transaction(|game, _| {
                    game.enter_level(sunrise_gameplay_common::game::Dim::Nether).unwrap();
                    let nether = game.level("minecraft:the_nether").unwrap();
                    let nether_committed = game.storage_transaction(|game, _| {
                        assert!(game.level_global_level_event(nether, 1038, pos, 0));
                        assert!(game.player_stop_sound(player13, Some(end_portal_spawn), None));
                        assert!(game.player_play_sound(
                            player13,
                            anvil,
                            SoundSource::Neutral,
                            1.0,
                            1.0
                        ));
                        assert!(game.level_level_event(nether, 1500, pos, 0));
                        assert!(game.entity_broadcast_event(entity13, 35));
                        assert!(
                            game.level_send_particles(nether, &heart, None, at, [0.0; 3], 0.0, 2)
                        );
                        assert!(game.level_play_sound(
                            nether,
                            bell,
                            at,
                            SoundSource::Blocks,
                            1.0,
                            1.0,
                            None
                        ));
                        Ok::<(), ()>(())
                    });
                    assert!(nether_committed.is_ok());
                    game.enter_level(sunrise_gameplay_common::game::Dim::Overworld).unwrap();
                    Ok::<(), ()>(())
                });
                assert!(committed.is_ok());
                assert_eq!(game.dim, sunrise_gameplay_common::game::Dim::Overworld);

                // Overworld guest 12 receives ONLY the global level event (1038); no Nether-local
                // level event, entity event, particles, or sounds leak into the Overworld.
                let ow_packets = decode_packets(&rx12);
                assert_eq!(
                    ow_packets.len(),
                    1,
                    "only global event 1038 reaches Overworld guest 12"
                );
                assert!(matches!(
                    ow_packets[0],
                    C::LevelEvent(ref e) if e.kind == 1038 && e.global
                ));

                // Nether guest 13 receives every packet on one connection in exact call order, and
                // `PlayerRef::play_sound` drew its seed from Nether's `world.random`.
                let expected_nether_seed = expected_nether_random.next_long();
                let nether_packets = decode_packets(&rx13);
                let tags: Vec<&'static str> = nether_packets
                    .iter()
                    .map(|p| match p {
                        C::LevelEvent(e) if e.kind == 1038 && e.global => "global_1038",
                        C::StopSound(s) if s.sound.as_deref() == Some(end_portal_spawn) => {
                            "stop_portal"
                        }
                        C::Sound(s) if s.sound.0 == anvil_id && s.seed == expected_nether_seed => {
                            "player_anvil"
                        }
                        C::LevelEvent(e) if e.kind == 1500 && !e.global => "level_1500",
                        C::EntityEvent(e) if e.id == entity13_id && e.event == 35 => "entity_35",
                        C::LevelParticles(p) if p.count == 2 => "particles_2",
                        C::Sound(s) if s.sound.0 == bell_id => "level_bell",
                        other => panic!("unexpected Nether packet: {other:?}"),
                    })
                    .collect();
                assert_eq!(
                    tags,
                    vec![
                        "global_1038",
                        "stop_portal",
                        "player_anvil",
                        "level_1500",
                        "entity_35",
                        "particles_2",
                        "level_bell",
                    ]
                );

                // Parked-level effects issued while Overworld is active and an Overworld
                // transaction is open replay into the Nether on Overworld commit in exact order
                // and stay discarded on abort.
                let nether_handle = game.level("minecraft:the_nether").unwrap();
                let cross_abort = game.storage_transaction(|game, _| {
                    assert!(game.level_send_particles(
                        nether_handle,
                        &heart,
                        None,
                        at,
                        [0.0; 3],
                        0.0,
                        2
                    ));
                    assert!(game.level_play_sound(
                        nether_handle,
                        bell,
                        at,
                        SoundSource::Blocks,
                        1.0,
                        1.0,
                        None
                    ));
                    Err::<(), ()>(())
                });
                assert!(cross_abort.is_err());
                assert!(decode_packets(&rx12).is_empty());
                assert!(decode_packets(&rx13).is_empty());

                let cross_commit = game.storage_transaction(|game, _| {
                    assert!(game.level_send_particles(
                        nether_handle,
                        &heart,
                        None,
                        at,
                        [0.0; 3],
                        0.0,
                        2
                    ));
                    assert!(game.level_play_sound(
                        nether_handle,
                        bell,
                        at,
                        SoundSource::Blocks,
                        1.0,
                        1.0,
                        None
                    ));
                    Ok::<(), ()>(())
                });
                assert!(cross_commit.is_ok());
                assert_eq!(game.dim, sunrise_gameplay_common::game::Dim::Overworld);
                assert!(decode_packets(&rx12).is_empty());
                let cross_tags: Vec<&'static str> = decode_packets(&rx13)
                    .iter()
                    .map(|p| match p {
                        C::LevelParticles(p) if p.count == 2 => "particles_2",
                        C::Sound(s) if s.sound.0 == bell_id => "level_bell",
                        other => panic!("unexpected parked-Nether packet: {other:?}"),
                    })
                    .collect();
                assert_eq!(cross_tags, vec!["particles_2", "level_bell"]);

                // Overworld's `world.random` must remain completely untouched by Nether's `play_sound`.
                assert_eq!(
                    game.world.random.clone().next_long(),
                    expected_overworld_random.next_long(),
                    "Nether PlayerRef::play_sound must not advance Overworld world.random"
                );

                // Packet order also holds inside an `as_guest` turn in BOTH directions
                // (`GlobalEvent(1038)` -> `StopSound` and `StopSound` -> `GlobalEvent(1038)`).
                game.enter_level(sunrise_gameplay_common::game::Dim::Nether).unwrap();
                let nether = game.level("minecraft:the_nether").unwrap();
                decode_packets(&rx13);
                game.as_guest(13, |game| {
                    let acting = game.player(13).unwrap();
                    let res = game.storage_transaction(|game, _| {
                        assert!(game.player_stop_sound(acting, Some(anvil), None));
                        assert!(game.level_global_level_event(nether, 1038, pos, 0));
                        assert!(game.player_stop_sound(acting, Some(end_portal_spawn), None));
                        Ok::<(), ()>(())
                    });
                    assert!(res.is_ok());
                })
                .expect("guest turn runs");
                let turn_tags: Vec<&'static str> = decode_packets(&rx13)
                    .into_iter()
                    .filter_map(|p| match p {
                        C::StopSound(s) if s.sound.as_deref() == Some(anvil) => Some("stop_anvil"),
                        C::LevelEvent(e) if e.kind == 1038 && e.global => Some("global_1038"),
                        C::StopSound(s) if s.sound.as_deref() == Some(end_portal_spawn) => {
                            Some("stop_portal")
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(turn_tags, vec!["stop_anvil", "global_1038", "stop_portal"]);
            });
        },
    );
}

use sunrise_gameplay_common::game::transactions::TransactionsGameExt as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_server::NativeServerGameExt1 as _;
