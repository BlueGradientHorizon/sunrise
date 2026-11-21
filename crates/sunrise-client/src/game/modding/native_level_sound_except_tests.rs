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
    use sunrise_core::random::{LegacyRandom, RandomSource};

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
        game.player = Some(
            sunrise_client::motion::PlayerMotion::new(
                game.motion,
                sunrise_core::geometry::Vec3::new(at[0], at[1], at[2]),
            )
            .unwrap(),
        );
        game.audio.set_listener(at, 0.0, 0.0);
        let end_portal_spawn = "minecraft:block.end_portal.spawn";
        let portal_plays =
            |game: &super::Game<'_>| game.audio.heard.get(end_portal_spawn).copied().unwrap_or(0);
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

        // Snapshot both RNG streams (hub sound_seeds and level world.random) so we can verify
        // that rolled-back transactions and aborted nested savepoints advance neither stream.
        game.lan
            .as_ref()
            .unwrap()
            .hub
            .set_sound_seeds(LegacyRandom::new(0x5A17_CAFE));
        let mut expected_sound_seeds = LegacyRandom::new(0x5A17_CAFE);
        let mut expected_level_random = game.world.random.clone();

        // 1. Rolled-back transaction discards every deferred effect path (with
        // global_sound_events = true) and leaves both RNG streams and End-portal playback count
        // untouched.
        let portal_before_abort = portal_plays(game);
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
            assert_eq!(portal_plays(game), portal_before_abort);
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
        assert_eq!(
            portal_plays(game),
            portal_before_abort,
            "rolled-back global_level_event must not increase local End-portal playback count"
        );
        assert!(decode_packets(&rx12).is_empty());
        assert!(decode_packets(&rx13).is_empty());
        assert_eq!(
            game.world.random.clone().next_long(),
            expected_level_random.clone().next_long(),
            "rolled-back transaction must not advance world.random"
        );

        // 2. Nested savepoints (global_sound_events = true): aborted child drops its effects
        // without advancing RNG; committed child waits for root commit, then publishes every
        // effect directly in call order.
        let portal_before_commit = portal_plays(game);
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
            assert_eq!(portal_plays(game), portal_before_commit);
            assert!(decode_packets(&rx12).is_empty());
            assert!(decode_packets(&rx13).is_empty());
            Ok::<(), ()>(())
        });
        assert!(committed.is_ok());

        // Committed effects deliver directly without waiting for `lan_flush_effects`.
        assert!(game.lan_events.is_empty());
        assert!(game.lan_global_events.is_empty());
        assert!(game.lan_entity_events.is_empty());
        assert_eq!(
            portal_plays(game),
            portal_before_commit + 1,
            "global_level_event plays its local global sound once on root commit"
        );

        let packets12 = decode_packets(&rx12);
        let packets13 = decode_packets(&rx13);
        let expected_burp_seed = expected_sound_seeds.next_long();
        let expected_bell_seed = expected_sound_seeds.next_long();
        let expected_anvil_seed = expected_level_random.next_long();

        let sounds12: Vec<_> = packets12
            .iter()
            .filter_map(|p| match p {
                C::Sound(s) => Some((s.sound.0, s.seed)),
                _ => None,
            })
            .collect();
        let sounds13: Vec<_> = packets13
            .iter()
            .filter_map(|p| match p {
                C::Sound(s) => Some((s.sound.0, s.seed)),
                _ => None,
            })
            .collect();
        assert_eq!(
            sounds12,
            vec![(burp_id, expected_burp_seed)],
            "entity_play_sound reaches nearby guest 12 only with the first sound seed"
        );
        assert_eq!(
            sounds13,
            vec![(anvil_id, expected_anvil_seed)],
            "guest 13 receives only player_play_sound with the first world.random seed"
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
                C::SoundEntity(s) => Some((s.sound.0, s.id, s.seed)),
                _ => None,
            })
            .collect();
        let bound13: Vec<_> = packets13
            .iter()
            .filter_map(|p| match p {
                C::SoundEntity(s) => Some((s.sound.0, s.id, s.seed)),
                _ => None,
            })
            .collect();
        assert_eq!(bound12, vec![(bell_id, entity13_id, expected_bell_seed)]);
        assert_eq!(bound13, vec![(bell_id, entity13_id, expected_bell_seed)]);
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
        // `level_event_at` fallback on rollback and emits it directly as a non-global LevelEvent
        // on commit.
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
        assert!(game.lan_events.is_empty());
        assert!(game.audio.capture.as_ref().unwrap().is_empty());
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

/// A transaction that emits Nether effects and commits while the Overworld is active replays each
/// effect in its recorded `Dim` (so Nether events/particles/sounds never leak to Overworld guests
/// and `PlayerRef::play_sound` advances Nether's `world.random` rather than the Overworld's) and
/// delivers packets on a single connection in exact call order across all effect kinds (including
/// `GlobalEvent(1038)` followed by `StopSound`, both outside and inside `as_guest`).
#[test]
#[ignore = "daily"]
fn cross_level_transaction_replays_effects_in_request_level_and_preserves_packet_order() {
    use sunrise_client::gamerules::Value;
    use sunrise_client::particles::{ParticleOptions, ParticleType};
    use sunrise_core::random::RandomSource;

    with_guests(|game| {
        game.options.dedicated = Some(Vec::new());
        game.audio.capture = Some(Vec::new());
        assert!(game.ui.rules.set("global_sound_events", Value::Bool(true)));
        let (tx12, rx12) = mpsc::channel();
        let (tx13, rx13) = mpsc::channel();
        game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
            g.sender = PlayerSender::new(tx12, Arc::default());
        });
        game.lan.as_mut().unwrap().hub.guests.update(1, |g| {
            g.sender = PlayerSender::new(tx13, Arc::default());
            g.dimension = "minecraft:the_nether".to_owned();
        });
        // Load Nether once so it sits in `levels.parked` when Overworld is active.
        game.enter_level(super::Dim::Nether).unwrap();
        let mut expected_nether_random = game.world.random.clone();
        game.enter_level(super::Dim::Overworld).unwrap();
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
            .lan
            .as_ref()
            .unwrap()
            .hub
            .registry_id("minecraft:sound_event", anvil)
            .unwrap();
        let bell_id = game
            .lan
            .as_ref()
            .unwrap()
            .hub
            .registry_id("minecraft:sound_event", bell)
            .unwrap();
        let heart = ParticleOptions::simple(ParticleType::Heart);
        decode_packets(&rx12);
        decode_packets(&rx13);

        // Open transaction in Overworld, switch to Nether to issue Nether effects across all
        // effect kinds, then switch back to Overworld before committing.
        let committed = mod_api::run_transaction(game, |game, _| {
            game.enter_level(super::Dim::Nether).unwrap();
            let nether = game.level("minecraft:the_nether").unwrap();
            assert!(game.level_global_level_event(nether, 1038, pos, 0));
            assert!(game.player_stop_sound(player13, Some(end_portal_spawn), None));
            assert!(game.player_play_sound(player13, anvil, SoundSource::Neutral, 1.0, 1.0));
            assert!(game.level_level_event(nether, 1500, pos, 0));
            assert!(game.entity_broadcast_event(entity13, 35));
            assert!(game.level_send_particles(nether, &heart, None, at, [0.0; 3], 0.0, 2));
            assert!(game.level_play_sound(nether, bell, at, SoundSource::Blocks, 1.0, 1.0, None));
            game.enter_level(super::Dim::Overworld).unwrap();
            Ok::<(), ()>(())
        });
        assert!(committed.is_ok());
        assert_eq!(game.dim, super::Dim::Overworld);

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
                C::StopSound(s) if s.sound.as_deref() == Some(end_portal_spawn) => "stop_portal",
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

        // Overworld's `world.random` must remain completely untouched by Nether's `play_sound`.
        assert_eq!(
            game.world.random.clone().next_long(),
            expected_overworld_random.next_long(),
            "Nether PlayerRef::play_sound must not advance Overworld world.random"
        );

        // Packet order also holds inside an `as_guest` turn in BOTH directions
        // (`GlobalEvent(1038)` -> `StopSound` and `StopSound` -> `GlobalEvent(1038)`).
        game.enter_level(super::Dim::Nether).unwrap();
        let nether = game.level("minecraft:the_nether").unwrap();
        decode_packets(&rx13);
        game.as_guest(13, |game| {
            let acting = game.player(13).unwrap();
            let res = mod_api::run_transaction(game, |game, _| {
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
}

/// Committed effects must not drain unrelated queued `lan_sounds`/`lan_particles`/event queues,
/// `play_level_sound_at_position` runs synchronously at the call inside a transaction (so commit
/// replay executes no mod code and leaves nothing piling up in any queue), and non-player
/// `living_hurt_and_break` keeps its equipment break event immediate so an aborted transaction
/// does not leave equipment broken without its break notification.
#[test]
#[ignore = "daily"]
fn committed_effects_leave_unrelated_queues_intact_prevent_sound_recursion_and_keep_mob_break_immediate()
 {
    use mod_api::{EquipmentSlot, Flow, HostSide, HostTransaction, LivingRef};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use sunrise_client::gamerules::Value;
    use sunrise_client::items::ItemStack;
    use sunrise_client::particles::{ParticleOptions, ParticleType};

    struct ReentrantSoundHook {
        calls: Arc<AtomicUsize>,
        triggered: Arc<AtomicBool>,
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for ReentrantSoundHook {
        fn play_level_sound_at_position(
            &self,
            host: HostSide<'_>,
            e: &mut mod_api::PlayLevelSoundAtPositionEvent,
        ) -> Flow {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if e.sound.to_string() == "minecraft:block.bell.use"
                && !self.triggered.swap(true, Ordering::SeqCst)
                && let HostSide::Server(server) = host
            {
                let res: Result<(), ()> = server.transaction(|server, _| {
                    assert!(e.level.play_sound(
                        server,
                        "minecraft:block.anvil.use",
                        e.position,
                        SoundSource::Blocks,
                        1.0,
                        1.0,
                        None,
                    ));
                    Ok(())
                });
                assert!(res.is_ok());
            }
            Flow::Continue
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let triggered = Arc::new(AtomicBool::new(false));
    let mods = vec![mod_api::ModEntry::new(
        "reentrant_sound_test",
        Box::new(ReentrantSoundHook {
            calls: Arc::clone(&calls),
            triggered: Arc::clone(&triggered),
        }),
    )]
    .into();

    mod_platform::testing::with_mods(mods, || {
        with_guests(|game| {
            game.options.dedicated = Some(Vec::new());
            game.audio.capture = Some(Vec::new());
            assert!(game.ui.rules.set("global_sound_events", Value::Bool(true)));
            let (tx12, rx12) = mpsc::channel();
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                g.sender = PlayerSender::new(tx12, Arc::default());
            });
            let level = game.level(game.dim.id()).unwrap();
            let player13 = game.player(13).unwrap();
            let entity13 = player13.entity();
            let entity13_id = i32::try_from(entity13.id()).unwrap();
            let at = [3.0, 64.0, 5.0];
            let pos = [3, 64, 5];
            let burp = "minecraft:entity.player.burp";
            let bell = "minecraft:block.bell.use";
            let anvil = "minecraft:block.anvil.use";
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

            // Part A: Queue unrelated effects outside any transaction first.
            assert!(game.level_play_sound(level, burp, at, SoundSource::Blocks, 1.0, 1.0, None));
            assert!(game.level_send_particles(level, &flame, None, at, [0.0; 3], 0.0, 4));
            assert!(game.level_level_event(level, 1000, pos, 0));
            assert!(game.level_global_level_event(level, 1028, pos, 0));
            assert!(game.entity_broadcast_event(entity13, 2));
            assert_eq!(game.creatures.lan_sounds.len(), 1);
            assert_eq!(game.creatures.lan_particles.len(), 1);
            assert_eq!(game.lan_events.len(), 1);
            assert_eq!(game.lan_global_events.len(), 1);
            assert_eq!(game.lan_entity_events, vec![(entity13_id, 2)]);
            decode_packets(&rx12);

            // Part B: Inside a transaction, `play_level_sound_at_position` runs synchronously at
            // the call (observing `bell`, emitting `anvil`, and observing `anvil`), while commit
            // performs pure delivery with zero mod callbacks and leaves unrelated queues intact.
            let mut calls_before_commit = 0;
            let committed = mod_api::run_transaction(game, |game, _| {
                assert!(game.level_play_sound(
                    level,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None
                ));
                assert!(game.level_send_particles(level, &heart, None, at, [0.0; 3], 0.0, 2));
                assert!(game.level_level_event(level, 1500, pos, 0));
                assert!(game.level_global_level_event(level, 1038, pos, 0));
                assert!(game.entity_broadcast_event(entity13, 35));
                calls_before_commit = calls.load(Ordering::SeqCst);
                assert_eq!(
                    calls_before_commit, 1,
                    "bell ran the listener at the call; nested anvil bypassed re-dispatch"
                );
                assert!(
                    sound_packets(&rx12).is_empty(),
                    "no packets are sent before outermost commit"
                );
                Ok::<(), ()>(())
            });
            assert!(committed.is_ok());

            // Unrelated queued effects must remain untouched on the normal flush path!
            assert_eq!(game.creatures.lan_sounds.len(), 1);
            assert_eq!(game.creatures.lan_sounds[0].sound, burp);
            assert_eq!(game.creatures.lan_particles.len(), 1);
            assert_eq!(game.creatures.lan_particles[0].count, 4);
            assert_eq!(game.lan_events.len(), 1);
            assert_eq!(game.lan_events[0].kind, 1000);
            assert_eq!(game.lan_global_events.len(), 1);
            assert_eq!(game.lan_global_events[0].1, 1028);
            assert_eq!(game.lan_entity_events, vec![(entity13_id, 2)]);

            // Commit replay ran zero mod callbacks and delivered both sounds in journal order.
            assert!(triggered.load(Ordering::SeqCst));
            assert_eq!(
                calls.load(Ordering::SeqCst),
                calls_before_commit,
                "commit replay runs no play_level_sound_at_* listeners"
            );
            let sounds12: Vec<i32> = sound_packets(&rx12)
                .into_iter()
                .map(|s| s.sound.0)
                .collect();
            assert_eq!(sounds12, vec![anvil_id, bell_id]);

            // Part C: Non-player `living_hurt_and_break` inside an aborted transaction mutates the
            // mob's equipment immediately (unjournaled), so its break event (49 for Head) must
            // also emit immediately rather than being discarded on rollback.
            game.lan_entity_events.clear();
            let zombie_id = game
                .spawn_mob(
                    "minecraft:zombie",
                    [3.5, 64.0, 5.5],
                    true,
                    super::creatures::SpawnSite::Raw,
                )
                .unwrap();
            let mut helmet = ItemStack::new("minecraft:iron_helmet", 1);
            let max_damage = game.items().stack_max_damage(&helmet);
            assert!(max_damage > 0, "iron helmet has durability");
            helmet.damage = max_damage - 1;
            game.mobs
                .iter_mut()
                .find(|m| m.id == zombie_id)
                .unwrap()
                .set_equipment_stack(EquipmentSlot::Head as usize, Some(helmet));
            let zombie_living = LivingRef::new(game.entity(zombie_id).unwrap());

            let aborted = mod_api::run_transaction(game, |game, _| {
                let broken = game.living_hurt_and_break(zombie_living, EquipmentSlot::Head, 1);
                assert!(broken.is_some(), "helmet breaks on final durability point");
                Err::<(), ()>(())
            });
            assert!(aborted.is_err());
            assert!(
                game.living_equipment(zombie_living, EquipmentSlot::Head)
                    .is_none(),
                "non-player equipment write stays broken after abort"
            );
            assert_eq!(
                game.lan_entity_events,
                vec![(zombie_id as i32, 49)],
                "coupled non-player break event stays on the immediate side of the rollback boundary"
            );
        });
    });
}

/// When a `play_level_sound_at_position` listener commits a transaction with mixed effect kinds
/// (playing a level sound and then stopping it for a player) or emits multiple follow-up sounds
/// from inside its hook, listeners run synchronously at the call, commit replay runs no mod code
/// with nothing piling up, mixed play/stop packet order holds on one connection, and rollback or
/// cancellation leaves both packets and `hub.sound_seeds` untouched.
#[test]
#[ignore = "daily"]
fn sound_listener_transaction_preserves_mixed_play_and_stop_packet_order() {
    use mod_api::{Flow, HostSide, HostTransaction, PlayerRef};
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use sunrise_core::random::{LegacyRandom, RandomSource};

    struct PlayThenStopHook {
        target: Arc<Mutex<Option<PlayerRef>>>,
        cancel_bell: Arc<AtomicBool>,
        emit_extra_burp: Arc<AtomicBool>,
        calls: Arc<AtomicUsize>,
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for PlayThenStopHook {
        fn play_level_sound_at_position(
            &self,
            host: HostSide<'_>,
            e: &mut mod_api::PlayLevelSoundAtPositionEvent,
        ) -> Flow {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if e.sound.to_string() == "minecraft:block.bell.use"
                && let HostSide::Server(server) = host
            {
                let player = self.target.lock().unwrap().expect("target player set");
                let res: Result<(), ()> = server.transaction(|server, _| {
                    assert!(e.level.play_sound(
                        server,
                        "minecraft:block.anvil.use",
                        e.position,
                        SoundSource::Blocks,
                        1.0,
                        1.0,
                        None,
                    ));
                    assert!(player.stop_sound(
                        server,
                        Some("minecraft:block.anvil.use"),
                        Some(SoundSource::Blocks),
                    ));
                    if self.emit_extra_burp.load(Ordering::SeqCst) {
                        assert!(e.level.play_sound(
                            server,
                            "minecraft:entity.player.burp",
                            e.position,
                            SoundSource::Players,
                            1.0,
                            1.0,
                            None,
                        ));
                    }
                    Ok(())
                });
                assert!(res.is_ok());
                if self.cancel_bell.load(Ordering::SeqCst) {
                    return Flow::Cancel;
                }
            }
            Flow::Continue
        }
    }

    let target = Arc::new(Mutex::new(None));
    let cancel_bell = Arc::new(AtomicBool::new(false));
    let emit_extra_burp = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let mods = vec![mod_api::ModEntry::new(
        "play_then_stop_sound_test",
        Box::new(PlayThenStopHook {
            target: Arc::clone(&target),
            cancel_bell: Arc::clone(&cancel_bell),
            emit_extra_burp: Arc::clone(&emit_extra_burp),
            calls: Arc::clone(&calls),
        }),
    )]
    .into();

    mod_platform::testing::with_mods(mods, || {
        with_guests(|game| {
            game.options.dedicated = Some(Vec::new());
            let (tx12, rx12) = mpsc::channel();
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                g.sender = PlayerSender::new(tx12, Arc::default());
            });
            let player12 = game.player(12).unwrap();
            *target.lock().unwrap() = Some(player12);
            let level = game.level(game.dim.id()).unwrap();
            let at = [3.0, 64.0, 5.0];
            let bell = "minecraft:block.bell.use";
            let anvil = "minecraft:block.anvil.use";
            let burp = "minecraft:entity.player.burp";
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
            let burp_id = game
                .lan
                .as_ref()
                .unwrap()
                .hub
                .registry_id("minecraft:sound_event", burp)
                .unwrap();
            decode_packets(&rx12);

            let classify = |packets: Vec<C>| -> Vec<&'static str> {
                packets
                    .into_iter()
                    .map(|p| match p {
                        C::Sound(s) if s.sound.0 == bell_id => "sound_bell",
                        C::Sound(s) if s.sound.0 == anvil_id => "sound_anvil",
                        C::Sound(s) if s.sound.0 == burp_id => "sound_burp",
                        C::StopSound(s) if s.sound.as_deref() == Some(anvil) => "stop_anvil",
                        other => panic!("unexpected packet: {other:?}"),
                    })
                    .collect()
            };

            // 1. Rolled-back outer transaction discards both the listener's committed nested
            // effects and the outer sound without advancing `hub.sound_seeds`.
            game.lan
                .as_ref()
                .unwrap()
                .hub
                .set_sound_seeds(LegacyRandom::new(0xBEEF_1234));
            let mut expected_seeds = LegacyRandom::new(0xBEEF_1234);
            let aborted = mod_api::run_transaction(game, |game, _| {
                assert!(game.level_play_sound(
                    level,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None
                ));
                Err::<(), ()>(())
            });
            assert!(aborted.is_err());
            assert!(decode_packets(&rx12).is_empty());
            assert!(game.creatures.lan_sounds.is_empty());

            // 2. Committed transaction: listener plays anvil, stops anvil, and plays burp inside
            // its callback; all resolve at the call, commit runs no mod callbacks and sends
            // `[sound_anvil, stop_anvil, sound_burp, sound_bell]` on one connection with nothing
            // left queued.
            emit_extra_burp.store(true, Ordering::SeqCst);
            calls.store(0, Ordering::SeqCst);
            let mut calls_at_closure_end = 0;
            let committed = mod_api::run_transaction(game, |game, _| {
                assert!(game.level_play_sound(
                    level,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None
                ));
                calls_at_closure_end = calls.load(Ordering::SeqCst);
                assert_eq!(
                    calls_at_closure_end, 1,
                    "bell ran the listener once; nested anvil and burp bypassed re-dispatch"
                );
                Ok::<(), ()>(())
            });
            assert!(committed.is_ok());
            assert_eq!(
                calls.load(Ordering::SeqCst),
                calls_at_closure_end,
                "commit replay runs no mod listeners"
            );
            assert!(game.creatures.lan_sounds.is_empty());
            let packets = decode_packets(&rx12);
            let sound_seeds: Vec<i64> = packets
                .iter()
                .filter_map(|p| match p {
                    C::Sound(s) => Some(s.seed),
                    _ => None,
                })
                .collect();
            assert_eq!(
                classify(packets),
                vec!["sound_anvil", "stop_anvil", "sound_burp", "sound_bell"]
            );
            assert_eq!(
                sound_seeds,
                vec![
                    expected_seeds.next_long(),
                    expected_seeds.next_long(),
                    expected_seeds.next_long(),
                ],
                "rollback left hub.sound_seeds untouched; committed sounds take draws 1..=3"
            );

            // 3. Listener canceling the outer bell sound suppresses bell (and draws no seed for
            // it) while keeping its own committed `[sound_anvil, stop_anvil]` in order.
            emit_extra_burp.store(false, Ordering::SeqCst);
            cancel_bell.store(true, Ordering::SeqCst);
            let replaced = mod_api::run_transaction(game, |game, _| {
                assert!(game.level_play_sound(
                    level,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None
                ));
                Ok::<(), ()>(())
            });
            assert!(replaced.is_ok());
            let replaced_packets = decode_packets(&rx12);
            let replaced_seeds: Vec<i64> = replaced_packets
                .iter()
                .filter_map(|p| match p {
                    C::Sound(s) => Some(s.seed),
                    _ => None,
                })
                .collect();
            assert_eq!(
                classify(replaced_packets),
                vec!["sound_anvil", "stop_anvil"]
            );
            assert_eq!(
                replaced_seeds,
                vec![expected_seeds.next_long()],
                "canceled outer bell draws no sound seed"
            );

            // 4. Non-transactional tick flush (`lan_sounds`): listener's committed transaction
            // delivers `[sound_anvil, stop_anvil]` cleanly before the outer bell sound with
            // nothing left piling up in `lan_sounds`.
            cancel_bell.store(false, Ordering::SeqCst);
            assert!(game.level_play_sound(level, bell, at, SoundSource::Blocks, 1.0, 1.0, None));
            game.flush_native_sounds();
            assert!(game.creatures.lan_sounds.is_empty());
            assert_eq!(
                classify(decode_packets(&rx12)),
                vec!["sound_anvil", "stop_anvil", "sound_bell"]
            );
        });
    });
}

/// Unconditional `play_level_sound_at_position` and `play_level_sound_at_entity` listeners that
/// play another sound every time they are called must not recurse infinitely or stall the tick —
/// neither inside nor outside a transaction. Each original sound dispatches its hook once
/// (`max_depth == 1`), produces nested sounds resolved without re-entering either level-sound
/// hook, and delivers packets in exact call order within the same tick/commit. A panicking
/// listener also releases the RAII guard so subsequent sounds still dispatch hooks.
#[test]
#[ignore = "daily"]
fn always_producing_sound_listener_stops_at_one_nested_sound_in_and_out_of_transactions() {
    use mod_api::{Flow, HostSide};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct AlwaysEmitHook {
        active_depth: Arc<AtomicUsize>,
        max_depth: Arc<AtomicUsize>,
        pos_calls: Arc<AtomicUsize>,
        entity_calls: Arc<AtomicUsize>,
        panic_once: Arc<AtomicBool>,
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for AlwaysEmitHook {
        fn play_level_sound_at_position(
            &self,
            host: HostSide<'_>,
            e: &mut mod_api::PlayLevelSoundAtPositionEvent,
        ) -> Flow {
            let depth = self.active_depth.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_depth.fetch_max(depth, Ordering::SeqCst);
            self.pos_calls.fetch_add(1, Ordering::SeqCst);
            if self.panic_once.swap(false, Ordering::SeqCst) {
                self.active_depth.fetch_sub(1, Ordering::SeqCst);
                panic!("simulated level-sound listener panic");
            }
            if let HostSide::Server(server) = host {
                assert!(e.level.play_sound(
                    server,
                    "minecraft:block.anvil.use",
                    e.position,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None,
                ));
            }
            self.active_depth.fetch_sub(1, Ordering::SeqCst);
            Flow::Continue
        }

        fn play_level_sound_at_entity(
            &self,
            host: HostSide<'_>,
            e: &mut mod_api::PlayLevelSoundAtEntityEvent,
        ) -> Flow {
            let depth = self.active_depth.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_depth.fetch_max(depth, Ordering::SeqCst);
            self.entity_calls.fetch_add(1, Ordering::SeqCst);
            if let HostSide::Server(server) = host {
                assert!(
                    e.entity
                        .play_sound(server, "minecraft:entity.player.burp", 1.0, 1.0,)
                );
                assert!(e.entity.play_bound_sound(
                    server,
                    "minecraft:block.anvil.use",
                    SoundSource::Players,
                    1.0,
                    1.0,
                ));
            }
            self.active_depth.fetch_sub(1, Ordering::SeqCst);
            Flow::Continue
        }
    }

    let active_depth = Arc::new(AtomicUsize::new(0));
    let max_depth = Arc::new(AtomicUsize::new(0));
    let pos_calls = Arc::new(AtomicUsize::new(0));
    let entity_calls = Arc::new(AtomicUsize::new(0));
    let panic_once = Arc::new(AtomicBool::new(false));
    let mods = vec![mod_api::ModEntry::new(
        "always_emit_sound_test",
        Box::new(AlwaysEmitHook {
            active_depth: Arc::clone(&active_depth),
            max_depth: Arc::clone(&max_depth),
            pos_calls: Arc::clone(&pos_calls),
            entity_calls: Arc::clone(&entity_calls),
            panic_once: Arc::clone(&panic_once),
        }),
    )]
    .into();

    mod_platform::testing::with_mods(mods, || {
        with_guests(|game| {
            game.options.dedicated = Some(Vec::new());
            let (tx12, rx12) = mpsc::channel();
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                g.sender = PlayerSender::new(tx12, Arc::default());
            });
            let level = game.level(game.dim.id()).unwrap();
            let entity13 = game.player(13).unwrap().entity();
            let at = [3.0, 64.0, 5.0];
            let bell = "minecraft:block.bell.use";
            let anvil = "minecraft:block.anvil.use";
            let burp = "minecraft:entity.player.burp";
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
            let burp_id = game
                .lan
                .as_ref()
                .unwrap()
                .hub
                .registry_id("minecraft:sound_event", burp)
                .unwrap();
            let classify = |packets: Vec<C>| -> Vec<&'static str> {
                packets
                    .into_iter()
                    .filter_map(|p| match p {
                        C::Sound(s) if s.sound.0 == bell_id => Some("pos_bell"),
                        C::Sound(s) if s.sound.0 == anvil_id => Some("pos_anvil"),
                        C::Sound(s) if s.sound.0 == burp_id => Some("pos_burp"),
                        C::SoundEntity(s) if s.sound.0 == bell_id => Some("ent_bell"),
                        C::SoundEntity(s) if s.sound.0 == anvil_id => Some("ent_anvil"),
                        _ => None,
                    })
                    .collect()
            };
            decode_packets(&rx12);

            // 1. Positional sound inside a transaction: `bell` dispatches `at_position` once,
            // which plays `anvil` without re-entering the hook; commit sends `[pos_anvil, pos_bell]`.
            let committed = mod_api::run_transaction(game, |game, _| {
                assert!(game.level_play_sound(
                    level,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None
                ));
                Ok::<(), ()>(())
            });
            assert!(committed.is_ok());
            assert_eq!(pos_calls.load(Ordering::SeqCst), 1);
            assert_eq!(entity_calls.load(Ordering::SeqCst), 0);
            assert_eq!(active_depth.load(Ordering::SeqCst), 0);
            assert_eq!(max_depth.load(Ordering::SeqCst), 1);
            assert!(game.creatures.lan_sounds.is_empty());
            assert_eq!(
                classify(decode_packets(&rx12)),
                vec!["pos_anvil", "pos_bell"]
            );

            // 2. Positional sound outside a transaction: one `flush_native_sounds` finishes the
            // tick, dispatches `at_position` once, delivers `[pos_anvil, pos_bell]` in call order,
            // and leaves `lan_sounds` empty.
            assert!(game.level_play_sound(level, bell, at, SoundSource::Blocks, 1.0, 1.0, None));
            game.flush_native_sounds();
            assert_eq!(pos_calls.load(Ordering::SeqCst), 2);
            assert_eq!(entity_calls.load(Ordering::SeqCst), 0);
            assert_eq!(active_depth.load(Ordering::SeqCst), 0);
            assert_eq!(max_depth.load(Ordering::SeqCst), 1);
            assert!(game.creatures.lan_sounds.is_empty());
            assert_eq!(
                classify(decode_packets(&rx12)),
                vec!["pos_anvil", "pos_bell"]
            );

            // 3. Entity-bound sound inside a transaction: dispatches `at_entity` once; its nested
            // positional (`burp`) and bound (`anvil`) sounds bypass BOTH level-sound hooks and
            // commit in call order `[pos_burp, ent_anvil, ent_bell]`.
            let entity_tx = mod_api::run_transaction(game, |game, _| {
                assert!(game.entity_play_bound_sound(
                    entity13,
                    bell,
                    SoundSource::Players,
                    1.0,
                    1.0
                ));
                Ok::<(), ()>(())
            });
            assert!(entity_tx.is_ok());
            assert_eq!(pos_calls.load(Ordering::SeqCst), 2);
            assert_eq!(entity_calls.load(Ordering::SeqCst), 1);
            assert_eq!(active_depth.load(Ordering::SeqCst), 0);
            assert_eq!(max_depth.load(Ordering::SeqCst), 1);
            assert!(game.creatures.lan_sounds.is_empty());
            assert_eq!(
                classify(decode_packets(&rx12)),
                vec!["pos_burp", "ent_anvil", "ent_bell"]
            );

            // 4. Entity-bound sound outside a transaction: one flush delivers the exact same
            // `[pos_burp, ent_anvil, ent_bell]` sequence with `lan_sounds` empty.
            assert!(game.entity_play_bound_sound(entity13, bell, SoundSource::Players, 1.0, 1.0));
            game.flush_native_sounds();
            assert_eq!(pos_calls.load(Ordering::SeqCst), 2);
            assert_eq!(entity_calls.load(Ordering::SeqCst), 2);
            assert_eq!(active_depth.load(Ordering::SeqCst), 0);
            assert_eq!(max_depth.load(Ordering::SeqCst), 1);
            assert!(game.creatures.lan_sounds.is_empty());
            assert_eq!(
                classify(decode_packets(&rx12)),
                vec!["pos_burp", "ent_anvil", "ent_bell"]
            );

            // 5. Panic safety: unwinding out of a listener releases the RAII guard so the next
            // sound still dispatches hooks normally.
            panic_once.store(true, Ordering::SeqCst);
            assert!(game.level_play_sound(level, bell, at, SoundSource::Blocks, 1.0, 1.0, None));
            let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                game.flush_native_sounds();
            }));
            assert!(panicked.is_err());
            assert!(!game.creatures.asking_level_sound);
            assert_eq!(pos_calls.load(Ordering::SeqCst), 3);
            decode_packets(&rx12);

            assert!(game.level_play_sound(level, bell, at, SoundSource::Blocks, 1.0, 1.0, None));
            game.flush_native_sounds();
            assert_eq!(pos_calls.load(Ordering::SeqCst), 4);
            assert_eq!(max_depth.load(Ordering::SeqCst), 1);
            assert!(game.creatures.lan_sounds.is_empty());
            assert_eq!(
                classify(decode_packets(&rx12)),
                vec!["pos_anvil", "pos_bell"]
            );
        });
    });
}

/// Regression for cross-level transactional writes during synchronous level-sound dispatch:
/// an Overworld transaction plays a sound in the Nether, whose listener writes a block at the
/// exact same coordinates in the Nether (directly or via a nested transaction merged into the
/// Overworld parent). Abort must restore both dimensions untouched; commit must apply only each
/// dimension's own writes without cross-dimension corruption or deduplication.
#[test]
#[ignore = "daily"]
fn cross_level_sound_listener_block_writes_journal_by_dimension_on_abort_commit_and_nested_merge() {
    use super::api_values::TryIntoApi;
    use mod_api::{BlockUpdateFlags, Flow, HostSide};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use sunrise_client::chunk::Chunk;

    struct NetherBlockHook {
        pos: [i32; 3],
        write_state: mod_api::BlockState,
        nested_tx: Arc<AtomicBool>,
        calls: Arc<AtomicUsize>,
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for NetherBlockHook {
        fn play_level_sound_at_position(
            &self,
            host: HostSide<'_>,
            e: &mut mod_api::PlayLevelSoundAtPositionEvent,
        ) -> Flow {
            if e.level.dimension_id() != "minecraft:the_nether" {
                return Flow::Continue;
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let HostSide::Server(server) = host {
                let write = |server: &mut dyn mod_api::ServerHost| {
                    assert_eq!(
                        e.level.set_block(
                            server,
                            self.pos,
                            self.write_state,
                            BlockUpdateFlags::NEIGHBORS,
                        ),
                        Ok(true)
                    );
                };
                if self.nested_tx.load(Ordering::SeqCst) {
                    let merged = mod_api::run_transaction(server, |server, _| {
                        write(server);
                        Ok::<(), ()>(())
                    });
                    assert!(merged.is_ok());
                } else {
                    write(server);
                }
            }
            Flow::Continue
        }
    }

    with_guests(|game| {
        game.options.dedicated = Some(Vec::new());
        let pos = [3, 64, 5];
        let at = [3.0, 64.0, 5.0];
        let bell = "minecraft:block.bell.use";
        let state = |game: &super::Game<'_>, name: &str| {
            game.pack
                .assets
                .registry
                .default_state(name)
                .unwrap_or_else(|| panic!("missing block state {name}"))
        };
        let diamond = state(game, "minecraft:diamond_block");
        let netherrack = state(game, "minecraft:netherrack");
        let emerald = state(game, "minecraft:emerald_block");
        let gold = state(game, "minecraft:gold_block");
        let emerald_api = emerald.try_into_api().unwrap();

        // Populate chunk (0, 0) in both dimensions with distinct blocks at the exact same coords.
        if !game.world.chunks.contains_key(&(0, 0)) {
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        }
        game.world.set(pos[0], pos[1], pos[2], diamond).unwrap();

        game.enter_level(super::Dim::Nether).unwrap();
        if !game.world.chunks.contains_key(&(0, 0)) {
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        }
        game.world.set(pos[0], pos[1], pos[2], netherrack).unwrap();
        game.enter_level(super::Dim::Overworld).unwrap();

        let overworld = game.level("minecraft:overworld").unwrap();
        let nether = game.level("minecraft:the_nether").unwrap();

        let block_in = |game: &mut super::Game<'_>, dim: super::Dim| {
            game.with_effect_level(dim, |g| g.world.block(pos[0], pos[1], pos[2]))
                .unwrap()
        };
        assert_eq!(block_in(game, super::Dim::Overworld), diamond);
        assert_eq!(block_in(game, super::Dim::Nether), netherrack);

        let nested_tx = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicUsize::new(0));
        let mods = vec![mod_api::ModEntry::new(
            "cross_level_sound_block_journal_test",
            Box::new(NetherBlockHook {
                pos,
                write_state: emerald_api,
                nested_tx: Arc::clone(&nested_tx),
                calls: Arc::clone(&calls),
            }),
        )]
        .into();

        mod_platform::testing::with_mods(mods, || {
            // 1. Direct listener write in Nether during an Overworld transaction that ABORTS:
            // neither dimension changes (Nether's old netherrack must not overwrite Overworld's
            // diamond_block, and Nether's emerald_block edit must roll back).
            let aborted = mod_api::run_transaction(game, |game, _| {
                assert!(game.level_play_sound(
                    nether,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None,
                ));
                assert_eq!(game.dim, super::Dim::Overworld);
                assert_eq!(block_in(game, super::Dim::Overworld), diamond);
                assert_eq!(block_in(game, super::Dim::Nether), emerald);
                Err::<(), ()>(())
            });
            assert!(aborted.is_err());
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(game.dim, super::Dim::Overworld);
            assert_eq!(block_in(game, super::Dim::Overworld), diamond);
            assert_eq!(block_in(game, super::Dim::Nether), netherrack);

            // 2. Direct listener write in Nether when the Overworld transaction ALSO edits the
            // same coordinates, then ABORTS: both dimensions restore their own original blocks.
            let aborted_both = mod_api::run_transaction(game, |game, _| {
                assert_eq!(
                    game.level_set_block(overworld, pos, gold, BlockUpdateFlags::NEIGHBORS),
                    Ok(true)
                );
                assert!(game.level_play_sound(
                    nether,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None,
                ));
                assert_eq!(block_in(game, super::Dim::Overworld), gold);
                assert_eq!(block_in(game, super::Dim::Nether), emerald);
                Err::<(), ()>(())
            });
            assert!(aborted_both.is_err());
            assert_eq!(calls.load(Ordering::SeqCst), 2);
            assert_eq!(block_in(game, super::Dim::Overworld), diamond);
            assert_eq!(block_in(game, super::Dim::Nether), netherrack);

            // 3. Direct listener write in Nether during an Overworld transaction that COMMITS:
            // Overworld stays diamond_block and only Nether changes to emerald_block.
            let committed = mod_api::run_transaction(game, |game, _| {
                assert!(game.level_play_sound(
                    nether,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None,
                ));
                Ok::<(), ()>(())
            });
            assert!(committed.is_ok());
            assert_eq!(calls.load(Ordering::SeqCst), 3);
            assert_eq!(game.dim, super::Dim::Overworld);
            assert_eq!(block_in(game, super::Dim::Overworld), diamond);
            assert_eq!(block_in(game, super::Dim::Nether), emerald);

            // Reset Nether back to netherrack for the nested-transaction merge cases.
            game.with_effect_level(super::Dim::Nether, |g| {
                g.world.set(pos[0], pos[1], pos[2], netherrack).unwrap();
            })
            .unwrap();

            // 4. Nested transaction in Nether merges into its Overworld parent, then the parent
            // ABORTS (even while Overworld also wrote to `pos`): both dimensions stay unchanged.
            nested_tx.store(true, Ordering::SeqCst);
            let nested_abort = mod_api::run_transaction(game, |game, _| {
                assert_eq!(
                    game.level_set_block(overworld, pos, gold, BlockUpdateFlags::NEIGHBORS),
                    Ok(true)
                );
                assert!(game.level_play_sound(
                    nether,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None,
                ));
                assert_eq!(block_in(game, super::Dim::Overworld), gold);
                assert_eq!(block_in(game, super::Dim::Nether), emerald);
                Err::<(), ()>(())
            });
            assert!(nested_abort.is_err());
            assert_eq!(calls.load(Ordering::SeqCst), 4);
            assert_eq!(block_in(game, super::Dim::Overworld), diamond);
            assert_eq!(block_in(game, super::Dim::Nether), netherrack);

            // 5. Nested transaction in Nether merges into its Overworld parent, then the parent
            // COMMITS: Overworld stays diamond_block and only Nether changes to emerald_block.
            let nested_commit = mod_api::run_transaction(game, |game, _| {
                assert!(game.level_play_sound(
                    nether,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None,
                ));
                Ok::<(), ()>(())
            });
            assert!(nested_commit.is_ok());
            assert_eq!(calls.load(Ordering::SeqCst), 5);
            assert_eq!(block_in(game, super::Dim::Overworld), diamond);
            assert_eq!(block_in(game, super::Dim::Nether), emerald);

            // 6. Nested Nether commit + same-coordinate Overworld write both commit without
            // deduplicating or clobbering each other across dimensions.
            game.with_effect_level(super::Dim::Nether, |g| {
                g.world.set(pos[0], pos[1], pos[2], netherrack).unwrap();
            })
            .unwrap();
            let both_commit = mod_api::run_transaction(game, |game, _| {
                assert_eq!(
                    game.level_set_block(overworld, pos, gold, BlockUpdateFlags::NEIGHBORS),
                    Ok(true)
                );
                assert!(game.level_play_sound(
                    nether,
                    bell,
                    at,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                    None,
                ));
                Ok::<(), ()>(())
            });
            assert!(both_commit.is_ok());
            assert_eq!(calls.load(Ordering::SeqCst), 6);
            assert_eq!(block_in(game, super::Dim::Overworld), gold);
            assert_eq!(block_in(game, super::Dim::Nether), emerald);
        });
    });
}
