//! Play-phase `post_effects` lifecycle and pre-login protocol error against 26.3.
use crate::game::{Game, native_entity_refs::tests::with_client_game};
use std::time::{Duration, Instant};
use sunrise_client::{multiplayer, net::Session, title::Mode};
use sunrise_protocol::{
    connection::{Connection, Pipe},
    packets::{
        Empty, configuration as cfg,
        play::{self, Clientbound as C, Serverbound as S, SpawnInfo},
    },
};

fn post_effects_witness() -> serde_json::Value {
    let dir = std::env::var("SUNRISE_NETWORK_WITNESS_DIR")
        .expect("SUNRISE_NETWORK_WITNESS_DIR must be set (run via tools/test_network_parity.py)");
    let path = std::path::Path::new(&dir).join("post_effects.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

fn overworld_spawn() -> SpawnInfo {
    SpawnInfo {
        dimension_type: 0,
        dimension: "minecraft:overworld".into(),
        seed: 0,
        game_type: 0,
        previous_game_type: None,
        debug: false,
        flat: true,
        last_death: None,
        portal_cooldown: 0,
        sea_level: 63,
    }
}

fn login_packet(player_id: i32) -> play::Login {
    play::Login {
        player_id,
        hardcore: false,
        levels: vec!["minecraft:overworld".into()],
        max_players: 20,
        chunk_radius: 10,
        simulation_distance: 10,
        reduced_debug_info: false,
        show_death_screen: true,
        limited_crafting: false,
        spawn: overworld_spawn(),
        online_mode: false,
        enforces_secure_chat: false,
    }
}

fn json_strings(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect()
}

fn wait_for_pong(game: &mut Game<'_>, peer: &mut Connection, ping_id: i32) {
    peer.send(&C::Ping(cfg::Ping { id: ping_id }).encode())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        game.poll_remote().unwrap();
        while let Ok((id, body)) = peer.recv() {
            if matches!(S::decode(id, &body), Ok(S::Pong(p)) if p.id == ping_id) {
                return;
            }
        }
        std::thread::yield_now();
    }
    panic!("timed out waiting for Pong({ping_id})");
}

#[test]
#[ignore = "run by tools/test_network_parity.py"]
fn post_effects_cleared_on_login_respawn_and_reconfigure_26_3() {
    let _guard = super::PLAY_LOOP_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let witness = post_effects_witness();
    let lifecycle = &witness["play_lifecycle"];

    with_client_game(|game| {
        let (client, _server) = Pipe::pair();
        game.connect_local(client, "PostEffectsLifecycle").unwrap();
        let (session, mut peer) = Session::play_loop_probe();
        let remote = game.remote.as_mut().unwrap();
        remote.session = session;
        remote.joined = true;
        remote.logged_in = false;
        peer.reader
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();

        // Stale post_effects must clear when initial Login creates the player.
        game.post_effects = vec!["minecraft:stale".into()];
        peer.send(&C::Login(login_packet(1)).encode()).unwrap();
        wait_for_pong(game, &mut peer, 1);
        assert_eq!(
            game.post_effects,
            json_strings(&lifecycle["after_initial_login"]),
            "after_initial_login"
        );

        let two = json_strings(&lifecycle["after_set_two"]);
        peer.send(
            &C::PostEffects(play::PostEffects {
                effects: two.clone(),
            })
            .encode(),
        )
        .unwrap();
        wait_for_pong(game, &mut peer, 2);
        assert_eq!(game.post_effects, two, "after_set_two");

        peer.send(&C::PostEffects(play::PostEffects { effects: vec![] }).encode())
            .unwrap();
        wait_for_pong(game, &mut peer, 3);
        assert_eq!(
            game.post_effects,
            json_strings(&lifecycle["after_set_empty"]),
            "after_set_empty"
        );

        // Respawn creates a fresh LocalPlayer with empty post_effects for every keep mask.
        for keep in 0..=3i8 {
            peer.send(
                &C::PostEffects(play::PostEffects {
                    effects: vec!["minecraft:invert".into()],
                })
                .encode(),
            )
            .unwrap();
            peer.send(
                &C::Respawn(play::Respawn {
                    spawn: overworld_spawn(),
                    keep,
                })
                .encode(),
            )
            .unwrap();
            wait_for_pong(game, &mut peer, 10 + i32::from(keep));
            let key = format!("keep_{keep}");
            assert_eq!(
                game.post_effects,
                json_strings(&lifecycle["after_respawn"][key.as_str()]),
                "after_respawn {key}"
            );
        }

        // Repeat Login without clearClientLevel keeps the existing LocalPlayer's post_effects.
        peer.send(
            &C::PostEffects(play::PostEffects {
                effects: vec!["minecraft:invert".into()],
            })
            .encode(),
        )
        .unwrap();
        peer.send(&C::Login(login_packet(2)).encode()).unwrap();
        wait_for_pong(game, &mut peer, 20);
        assert_eq!(
            game.post_effects,
            json_strings(&lifecycle["after_repeat_login_without_clear"]),
            "after_repeat_login_without_clear"
        );

        // Reconfiguration runs clearClientLevel, so the next Login starts with empty post_effects.
        peer.send(&C::StartConfiguration(Empty).encode()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut acked = false;
        while Instant::now() < deadline && !acked {
            game.poll_remote().unwrap();
            while let Ok((id, body)) = peer.recv() {
                if matches!(S::decode(id, &body), Ok(S::ConfigurationAcknowledged(_))) {
                    acked = true;
                    break;
                }
            }
            std::thread::yield_now();
        }
        assert!(acked, "never received ConfigurationAcknowledged");

        peer.send(&cfg::Clientbound::FinishConfiguration(Empty).encode())
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut finished = false;
        while Instant::now() < deadline && !finished {
            game.poll_remote().unwrap();
            while let Ok((id, body)) = peer.recv() {
                if matches!(
                    cfg::Serverbound::decode(id, &body),
                    Ok(cfg::Serverbound::FinishConfiguration(_))
                ) {
                    finished = true;
                    break;
                }
            }
            std::thread::yield_now();
        }
        assert!(finished, "never received FinishConfiguration");

        peer.send(&C::Login(login_packet(3)).encode()).unwrap();
        wait_for_pong(game, &mut peer, 30);
        assert_eq!(
            game.post_effects,
            json_strings(&lifecycle["after_reconfigure_then_login"]),
            "after_reconfigure_then_login"
        );
    });
}

#[test]
#[ignore = "run by tools/test_network_parity.py"]
fn post_effects_before_login_disconnects_with_packet_error_26_3() {
    let _guard = super::PLAY_LOOP_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let witness = post_effects_witness();
    for (case_name, effects) in [
        ("empty", Vec::<String>::new()),
        ("nonempty", vec!["minecraft:invert".to_owned()]),
    ] {
        let expected_key = witness["play_before_login"][case_name]["disconnect_key"]
            .as_str()
            .unwrap();
        with_client_game(|game| {
            game.title = None;
            game.post_effects.clear();
            let (client, _server) = Pipe::pair();
            game.connect_local(client, "PostEffectsBeforeLogin")
                .unwrap();
            let (session, mut peer) = Session::play_loop_probe();
            let remote = game.remote.as_mut().unwrap();
            remote.session = session;
            remote.joined = true;
            remote.logged_in = false;
            peer.reader
                .set_read_timeout(Some(Duration::from_millis(20)))
                .unwrap();

            peer.send(&C::PostEffects(play::PostEffects { effects }).encode())
                .unwrap();
            peer.send(&C::Ping(cfg::Ping { id: 77 }).encode()).unwrap();

            let deadline = Instant::now() + Duration::from_secs(5);
            let mut saw_pong = false;
            while game.remote.is_some() && !saw_pong && Instant::now() < deadline {
                game.poll_remote().unwrap();
                while let Ok((id, body)) = peer.recv() {
                    if matches!(S::decode(id, &body), Ok(S::Pong(p)) if p.id == 77) {
                        saw_pong = true;
                        break;
                    }
                }
                std::thread::yield_now();
            }
            assert!(
                game.remote.is_none(),
                "play_before_login ({case_name}): post_effects before Login did not disconnect"
            );
            assert!(
                game.post_effects.is_empty(),
                "play_before_login ({case_name}): post_effects were applied before Login"
            );
            let Some(Mode::Multiplayer(multiplayer::Screen::Disconnected { reason, .. })) =
                game.title.as_ref().map(|t| &t.mode)
            else {
                panic!("play_before_login ({case_name}): missing Disconnected screen");
            };
            assert_eq!(
                reason.translation_key(),
                Some(expected_key),
                "play_before_login ({case_name})"
            );
        });
    }
}
