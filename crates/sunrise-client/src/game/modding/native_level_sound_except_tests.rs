//! `LevelRef::play_sound` leaving one player out: the player handle is translated to the
//! connection inside the game.

use super::native_guest_turn_tests::with_guests;
use mod_api::SoundSource;
use std::sync::{Arc, mpsc};
use sunrise_client::lan::{Out, PlayerSender};
use sunrise_protocol::{buf::Reader, packets::play::Clientbound as C};

fn decode_packets(rx: &mpsc::Receiver<Out>) -> Vec<C> {
    rx.try_iter()
        .flat_map(|out| match out {
            Out::Packet(body) => vec![body],
            Out::Packets(bodies) => bodies,
            _ => Vec::new(),
        })
        .filter_map(|body| {
            let mut reader = Reader::new(&body);
            let id = reader.varint().unwrap();
            C::decode(id, reader.rest()).ok()
        })
        .collect()
}

fn sound_packets(rx: &mpsc::Receiver<Out>) -> Vec<sunrise_protocol::packets::play::Sound> {
    decode_packets(rx)
        .into_iter()
        .filter_map(|packet| match packet {
            C::Sound(sound) => Some(sound),
            _ => None,
        })
        .collect()
}

/// A sound queued with `except` names that player's connection; without it, no one.
#[test]
#[ignore = "daily"]
fn a_level_sound_leaves_out_the_player_asked() {
    with_guests(|game| {
        game.options.dedicated = Some(Vec::new());
        let player = game.player(13).unwrap();
        let level = game.level(game.dim.id()).unwrap();
        let at = [0.0, 64.0, 0.0];
        let sound = "minecraft:block.bell.use";
        let play = |game: &mut super::Game<'_>, except| {
            game.level_play_sound(level, sound, at, SoundSource::Blocks, 1.0, 1.0, except)
        };
        let queued = |game: &super::Game<'_>| game.creatures.lan_sounds.len();
        let before = queued(game);
        assert!(play(game, Some(player)));
        assert_eq!(game.creatures.lan_sounds.last().unwrap().except, Some(13));
        assert!(play(game, None));
        assert_eq!(game.creatures.lan_sounds.last().unwrap().except, None);
        assert_eq!(queued(game), before + 2);
        let gone = mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(
                mod_api::WorldEpoch::new(player.entity().epoch().get() + 1),
                player.entity().id(),
            ),
            player.uuid(),
        );
        assert!(!play(game, Some(gone)), "a stale player queues nothing");
        assert_eq!(queued(game), before + 2);
    });
}

/// Sounds played through `LevelRef::play_sound` inside an open transaction are held until root
/// commit and discarded on rollback, including nested savepoints, while preserving `except` and
/// argument validation.
#[test]
#[ignore = "daily"]
fn a_level_sound_inside_a_transaction_waits_for_commit_and_discards_on_rollback() {
    with_guests(|game| {
        game.options.dedicated = Some(Vec::new());
        let (tx12, rx12) = mpsc::channel();
        let (tx13, rx13) = mpsc::channel();
        game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
            g.sender = PlayerSender::new(tx12, Arc::default());
        });
        game.lan.as_mut().unwrap().hub.guests.update(1, |g| {
            g.sender = PlayerSender::new(tx13, Arc::default());
        });
        let player13 = game.player(13).unwrap();
        let level = game.level(game.dim.id()).unwrap();
        let at = [3.0, 64.0, 5.0];
        let bell = "minecraft:block.bell.use";
        let anvil = "minecraft:block.anvil.use";
        let gone = mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(
                mod_api::WorldEpoch::new(player13.entity().epoch().get() + 1),
                player13.entity().id(),
            ),
            player13.uuid(),
        );
        let bell_id = game
            .lan
            .as_ref()
            .unwrap()
            .hub
            .registry_id("minecraft:sound_event", bell)
            .unwrap();
        let anvil_id = game
            .lan
            .as_ref()
            .unwrap()
            .hub
            .registry_id("minecraft:sound_event", anvil)
            .unwrap();

        // Rolled-back transaction: nothing is queued while open and nothing publishes on abort.
        let rolled = mod_api::run_transaction(game, |game, _| {
            assert!(game.level_play_sound(level, bell, at, SoundSource::Blocks, 1.0, 1.0, None));
            assert!(!game.level_play_sound(
                level,
                bell,
                at,
                SoundSource::Blocks,
                1.0,
                1.0,
                Some(gone)
            ));
            assert!(
                game.creatures.lan_sounds.is_empty(),
                "deferred sound must not queue before commit"
            );
            assert!(sound_packets(&rx12).is_empty());
            assert!(sound_packets(&rx13).is_empty());
            Err::<(), ()>(())
        });
        assert!(rolled.is_err());
        assert!(game.creatures.lan_sounds.is_empty());
        assert!(sound_packets(&rx12).is_empty());
        assert!(sound_packets(&rx13).is_empty());

        // Nested transactions: an aborted child drops its sound; a committed child waits for root
        // commit and respects `except`.
        let committed = mod_api::run_transaction(game, |game, _| {
            let inner_abort = mod_api::run_transaction(game, |game, _| {
                assert!(game.level_play_sound(
                    level,
                    anvil,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None
                ));
                Err::<(), ()>(())
            });
            assert!(inner_abort.is_err());

            let inner_commit = mod_api::run_transaction(game, |game, _| {
                assert!(game.level_play_sound(
                    level,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    Some(player13)
                ));
                Ok::<(), ()>(())
            });
            assert!(inner_commit.is_ok());
            // Still inside the outer transaction: neither queued nor sent yet.
            assert!(game.creatures.lan_sounds.is_empty());
            assert!(sound_packets(&rx12).is_empty());
            assert!(sound_packets(&rx13).is_empty());
            Ok::<(), ()>(())
        });
        assert!(committed.is_ok());

        let sounds12 = sound_packets(&rx12);
        let sounds13 = sound_packets(&rx13);
        assert_eq!(sounds12.len(), 1, "only the committed bell sound publishes");
        assert_eq!(sounds12[0].sound.0, bell_id);
        assert_ne!(sounds12[0].sound.0, anvil_id);
        assert!(
            sounds13.is_empty(),
            "committed deferred sound still leaves out `except`"
        );

        // Outside a transaction, `level_play_sound` keeps queueing immediately as before.
        let before = game.creatures.lan_sounds.len();
        assert!(game.level_play_sound(level, anvil, at, SoundSource::Blocks, 1.0, 1.0, None));
        assert_eq!(game.creatures.lan_sounds.len(), before + 1);
    });
}

/// Entity sounds (`EntityRef::play_sound`), entity-bound sounds (`EntityRef::play_bound_sound`),
/// player sounds/stops (`PlayerRef::play_sound` / `stop_sound`), entity events
/// (`EntityRef::broadcast_event`), particles (`LevelRef::send_particles`), level events
/// (`LevelRef::level_event`), and global level events (`LevelRef::global_level_event`, with
/// `global_sound_events` both on and off) all route through the shared deferred `PendingWrite`
/// path inside open transactions, discarding on rollback or aborted savepoint and publishing only
/// on root commit.
#[test]
#[ignore = "daily"]
fn entity_sounds_bound_sounds_particles_and_level_events_defer_inside_transactions() {
    use sunrise_client::gamerules::Value;
    use sunrise_client::particles::{ParticleOptions, ParticleType};

    with_guests(|game| {
        game.options.dedicated = Some(Vec::new());
        game.audio.capture = Some(Vec::new());
        let (tx12, rx12) = mpsc::channel();
        let (tx13, rx13) = mpsc::channel();
        game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
            g.sender = PlayerSender::new(tx12, Arc::default());
        });
        game.lan.as_mut().unwrap().hub.guests.update(1, |g| {
            g.sender = PlayerSender::new(tx13, Arc::default());
        });
        let player13 = game.player(13).unwrap();
        let entity13 = player13.entity();
        let entity13_id = i32::try_from(entity13.id()).unwrap();
        let level = game.level(game.dim.id()).unwrap();
        let at = [3.0, 64.0, 5.0];
        let pos = [3, 64, 5];
        let burp = "minecraft:entity.player.burp";
        let bell = "minecraft:block.bell.use";
        let anvil = "minecraft:block.anvil.use";
        let burp_id = game
            .lan
            .as_ref()
            .unwrap()
            .hub
            .registry_id("minecraft:sound_event", burp)
            .unwrap();
        let bell_id = game
            .lan
            .as_ref()
            .unwrap()
            .hub
            .registry_id("minecraft:sound_event", bell)
            .unwrap();
        let anvil_id = game
            .lan
            .as_ref()
            .unwrap()
            .hub
            .registry_id("minecraft:sound_event", anvil)
            .unwrap();
        let heart = ParticleOptions::simple(ParticleType::Heart);
        let flame = ParticleOptions::simple(ParticleType::Flame);

        let clear_queues = |game: &mut super::Game<'_>| {
            game.creatures.lan_sounds.clear();
            game.creatures.lan_particles.clear();
            game.level_events.clear();
            game.lan_events.clear();
            game.lan_global_events.clear();
            game.lan_entity_events.clear();
            if let Some(captured) = game.audio.capture.as_mut() {
                captured.clear();
            }
        };
        clear_queues(game);
        decode_packets(&rx12);
        decode_packets(&rx13);

        // 1. Rolled-back transaction discards every deferred effect path (with
        // global_sound_events = true).
        assert!(game.ui.rules.set("global_sound_events", Value::Bool(true)));
        let rolled = mod_api::run_transaction(game, |game, _| {
            assert!(game.entity_play_sound(entity13, burp, 1.0, 1.0));
            assert!(game.entity_play_bound_sound(entity13, bell, SoundSource::Players, 1.0, 1.0));
            assert!(game.player_play_sound(player13, anvil, SoundSource::Neutral, 1.0, 1.0));
            assert!(game.player_stop_sound(player13, Some(anvil), Some(SoundSource::Neutral)));
            assert!(game.entity_broadcast_event(entity13, 35));
            assert!(game.level_send_particles(level, &heart, None, at, [0.0; 3], 0.0, 2));
            assert!(game.level_level_event(level, 1500, pos, 0));
            assert!(game.level_global_level_event(level, 1038, pos, 0));
            assert!(game.creatures.lan_sounds.is_empty());
            assert!(game.creatures.lan_particles.is_empty());
            assert!(game.lan_events.is_empty());
            assert!(game.lan_global_events.is_empty());
            assert!(game.lan_entity_events.is_empty());
            assert!(game.audio.capture.as_ref().unwrap().is_empty());
            assert!(decode_packets(&rx12).is_empty());
            assert!(decode_packets(&rx13).is_empty());
            Err::<(), ()>(())
        });
        assert!(rolled.is_err());
        assert!(game.creatures.lan_sounds.is_empty());
        assert!(game.creatures.lan_particles.is_empty());
        assert!(game.lan_events.is_empty());
        assert!(game.lan_global_events.is_empty());
        assert!(game.lan_entity_events.is_empty());
        assert!(game.audio.capture.as_ref().unwrap().is_empty());
        assert!(decode_packets(&rx12).is_empty());
        assert!(decode_packets(&rx13).is_empty());

        // 2. Nested savepoints (global_sound_events = true): aborted child drops its effects;
        // committed child waits for root commit, then publishes every effect path cleanly.
        let committed = mod_api::run_transaction(game, |game, _| {
            let aborted_child = mod_api::run_transaction(game, |game, _| {
                assert!(game.entity_play_sound(entity13, anvil, 1.0, 1.0));
                assert!(game.entity_play_bound_sound(
                    entity13,
                    anvil,
                    SoundSource::Players,
                    1.0,
                    1.0
                ));
                assert!(game.player_play_sound(player13, burp, SoundSource::Neutral, 1.0, 1.0));
                assert!(game.player_stop_sound(player13, Some(burp), Some(SoundSource::Neutral)));
                assert!(game.entity_broadcast_event(entity13, 2));
                assert!(game.level_send_particles(level, &flame, None, at, [0.0; 3], 0.0, 4));
                assert!(game.level_level_event(level, 1000, pos, 0));
                assert!(game.level_global_level_event(level, 1028, pos, 0));
                Err::<(), ()>(())
            });
            assert!(aborted_child.is_err());

            let kept_child = mod_api::run_transaction(game, |game, _| {
                assert!(game.entity_play_sound(entity13, burp, 1.0, 1.0));
                assert!(game.entity_play_bound_sound(
                    entity13,
                    bell,
                    SoundSource::Players,
                    1.0,
                    1.0
                ));
                assert!(game.player_play_sound(player13, anvil, SoundSource::Neutral, 1.0, 1.0));
                assert!(game.player_stop_sound(player13, Some(anvil), Some(SoundSource::Neutral)));
                assert!(game.entity_broadcast_event(entity13, 35));
                assert!(game.level_send_particles(level, &heart, None, at, [0.0; 3], 0.0, 3));
                assert!(game.level_level_event(level, 1500, pos, 0));
                assert!(game.level_global_level_event(level, 1038, pos, 0));
                Ok::<(), ()>(())
            });
            assert!(kept_child.is_ok());

            // Nothing published before outermost commit.
            assert!(game.creatures.lan_sounds.is_empty());
            assert!(game.creatures.lan_particles.is_empty());
            assert!(game.lan_events.is_empty());
            assert!(game.lan_global_events.is_empty());
            assert!(game.lan_entity_events.is_empty());
            assert!(game.audio.capture.as_ref().unwrap().is_empty());
            assert!(decode_packets(&rx12).is_empty());
            assert!(decode_packets(&rx13).is_empty());
            Ok::<(), ()>(())
        });
        assert!(committed.is_ok());

        assert_eq!(
            game.lan_events,
            vec![super::QueuedLevelEvent {
                kind: 1500,
                pos,
                data: 0,
                source: None,
            }]
        );
        assert_eq!(
            game.lan_global_events,
            vec![("minecraft:overworld", 1038, pos, 0)]
        );
        assert_eq!(game.lan_entity_events, vec![(entity13_id, 35)]);
        assert!(
            !game.audio.capture.as_ref().unwrap().is_empty(),
            "global_level_event plays its local global sound on root commit"
        );

        game.lan_flush_effects();
        let packets12 = decode_packets(&rx12);
        let packets13 = decode_packets(&rx13);
        let sounds12: Vec<_> = packets12
            .iter()
            .filter_map(|p| match p {
                C::Sound(s) => Some(s.sound.0),
                _ => None,
            })
            .collect();
        let sounds13: Vec<_> = packets13
            .iter()
            .filter_map(|p| match p {
                C::Sound(s) => Some(s.sound.0),
                _ => None,
            })
            .collect();
        assert_eq!(
            sounds12,
            vec![burp_id],
            "entity_play_sound reaches nearby guest 12 only"
        );
        assert_eq!(
            sounds13,
            vec![anvil_id],
            "guest 13 receives only player_play_sound (not its own entity_play_sound)"
        );
        let stops12: Vec<_> = packets12
            .iter()
            .filter(|p| matches!(p, C::StopSound(_)))
            .collect();
        let stops13: Vec<_> = packets13
            .iter()
            .filter_map(|p| match p {
                C::StopSound(s) => Some(s.sound.as_deref()),
                _ => None,
            })
            .collect();
        assert!(stops12.is_empty());
        assert_eq!(stops13, vec![Some(anvil)]);
        let bound12: Vec<_> = packets12
            .iter()
            .filter_map(|p| match p {
                C::SoundEntity(s) => Some((s.sound.0, s.id)),
                _ => None,
            })
            .collect();
        let bound13: Vec<_> = packets13
            .iter()
            .filter_map(|p| match p {
                C::SoundEntity(s) => Some((s.sound.0, s.id)),
                _ => None,
            })
            .collect();
        assert_eq!(bound12, vec![(bell_id, entity13_id)]);
        assert_eq!(bound13, vec![(bell_id, entity13_id)]);
        let particles12: Vec<_> = packets12
            .iter()
            .filter_map(|p| match p {
                C::LevelParticles(p) => Some(p.count),
                _ => None,
            })
            .collect();
        let particles13: Vec<_> = packets13
            .iter()
            .filter_map(|p| match p {
                C::LevelParticles(p) => Some(p.count),
                _ => None,
            })
            .collect();
        assert_eq!(particles12, vec![3]);
        assert_eq!(particles13, vec![3]);
        let level_events12: Vec<_> = packets12
            .iter()
            .filter_map(|p| match p {
                C::LevelEvent(e) => Some((e.kind, e.global)),
                _ => None,
            })
            .collect();
        let level_events13: Vec<_> = packets13
            .iter()
            .filter_map(|p| match p {
                C::LevelEvent(e) => Some((e.kind, e.global)),
                _ => None,
            })
            .collect();
        assert_eq!(level_events12, vec![(1500, false), (1038, true)]);
        assert_eq!(level_events13, vec![(1500, false), (1038, true)]);
        let entity_events13: Vec<_> = packets13
            .iter()
            .filter_map(|p| match p {
                C::EntityEvent(e) => Some((e.id, e.event)),
                _ => None,
            })
            .collect();
        assert_eq!(entity_events13, vec![(entity13_id, 35)]);

        // 3. `LevelRef::global_level_event` with `global_sound_events = false` defers its plain
        // `level_event_at` fallback on rollback and emits it as a non-global LevelEvent on commit.
        assert!(game.ui.rules.set("global_sound_events", Value::Bool(false)));
        clear_queues(game);
        decode_packets(&rx12);
        decode_packets(&rx13);
        let rolled_plain = mod_api::run_transaction(game, |game, _| {
            assert!(game.level_global_level_event(level, 1023, pos, 0));
            assert!(game.lan_events.is_empty());
            assert!(game.lan_global_events.is_empty());
            assert!(game.audio.capture.as_ref().unwrap().is_empty());
            Err::<(), ()>(())
        });
        assert!(rolled_plain.is_err());
        assert!(game.lan_events.is_empty());
        assert!(game.lan_global_events.is_empty());
        assert!(game.audio.capture.as_ref().unwrap().is_empty());

        let committed_plain = mod_api::run_transaction(game, |game, _| {
            assert!(game.level_global_level_event(level, 1023, pos, 0));
            assert!(game.lan_events.is_empty());
            Ok::<(), ()>(())
        });
        assert!(committed_plain.is_ok());
        assert!(game.lan_global_events.is_empty());
        assert!(game.audio.capture.as_ref().unwrap().is_empty());
        assert_eq!(
            game.lan_events,
            vec![super::QueuedLevelEvent {
                kind: 1023,
                pos,
                data: 0,
                source: None,
            }]
        );
        game.lan_flush_effects();
        let plain12: Vec<_> = decode_packets(&rx12)
            .into_iter()
            .filter_map(|p| match p {
                C::LevelEvent(e) => Some((e.kind, e.global)),
                _ => None,
            })
            .collect();
        assert_eq!(plain12, vec![(1023, false)]);
        decode_packets(&rx13);

        // 4. Outside a transaction, every effect path continues queueing/sending immediately.
        assert!(game.ui.rules.set("global_sound_events", Value::Bool(true)));
        clear_queues(game);
        assert!(game.entity_play_sound(entity13, burp, 1.0, 1.0));
        assert!(game.entity_play_bound_sound(entity13, bell, SoundSource::Players, 1.0, 1.0));
        assert!(game.player_play_sound(player13, anvil, SoundSource::Neutral, 1.0, 1.0));
        assert!(game.player_stop_sound(player13, Some(anvil), Some(SoundSource::Neutral)));
        assert!(game.entity_broadcast_event(entity13, 35));
        assert!(game.level_send_particles(level, &heart, None, at, [0.0; 3], 0.0, 1));
        assert!(game.level_level_event(level, 1500, pos, 0));
        assert!(game.level_global_level_event(level, 1038, pos, 0));
        assert_eq!(game.creatures.lan_sounds.len(), 2);
        assert_eq!(game.creatures.lan_sounds[0].entity, None);
        assert_eq!(game.creatures.lan_sounds[1].entity, Some(entity13_id));
        assert_eq!(game.creatures.lan_particles.len(), 1);
        assert_eq!(game.lan_events.len(), 1);
        assert_eq!(game.lan_global_events.len(), 1);
        assert_eq!(game.lan_entity_events, vec![(entity13_id, 35)]);
        let direct13 = decode_packets(&rx13);
        assert!(direct13.iter().any(|p| matches!(p, C::Sound(_))));
        assert!(direct13.iter().any(|p| matches!(p, C::StopSound(_))));
    });
}
