use crate::game::remote::movement_input_tests::local_rotation_tests::sent_input;
use crate::game::remote::movement_input_tests::local_rotation_tests::sent_moves;
use crate::game::remote::movement_input_tests::local_rotation_tests::sent_packets;
use crate::game::server_side::native_entity_refs::tests::with_client_game;
use mod_api::LocalRotationError;
use sunrise_core::geometry::Vec3;
use sunrise_protocol::connection::Connection;
use sunrise_sim::motion::PlayerMotion;
use {
    sunrise_client_gameplay::game::client_side::native_client_session::tests::finish_login,
    sunrise_client_gameplay::game::client_side::native_client_session::tests::logged_client,
};

#[test]
#[ignore = "daily"]
fn a_mod_set_look_is_clamped_and_sent_once_by_the_tick() {
    mod_platform::testing::with_mods(Vec::new().into(), || {
        with_client_game(|game| {
            assert_eq!(
                game.set_local_rotation(10.0, 5.0),
                Err(LocalRotationError::NoWorld)
            );
            let mut server = Connection::local(logged_client(game));
            server
                .reader
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            finish_login(game, &mut server);
            game.client.title.0 = None;
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
            // The baseline: the first sync sends the position.
            game.remote_send_state(0);
            sent_moves(game, &mut server);

            // Yaw stays unwrapped, pitch is clamped; the call itself sends nothing.
            assert_eq!(game.set_local_rotation(450.0, 120.0), Ok(()));
            assert_eq!((game.yaw, game.pitch), (450.0, 90.0));
            assert!(sent_moves(game, &mut server).is_empty());
            for (yaw, pitch) in [
                (f32::NAN, 0.0),
                (0.0, f32::INFINITY),
                (f32::NEG_INFINITY, 0.0),
            ] {
                assert_eq!(
                    game.set_local_rotation(yaw, pitch),
                    Err(LocalRotationError::NotFinite)
                );
                assert_eq!((game.yaw, game.pitch), (450.0, 90.0), "unchanged");
            }
            game.remote_send_state(0);
            assert_eq!(sent_moves(game, &mut server), [("rot", 450.0, 90.0)]);

            // Two calls in one tick: one packet, the last values.
            game.set_local_rotation(30.0, -10.0).unwrap();
            game.set_local_rotation(-200.0, 45.0).unwrap();
            game.remote_send_state(0);
            assert_eq!(sent_moves(game, &mut server), [("rot", -200.0, 45.0)]);
            game.remote_send_state(0);
            assert!(sent_moves(game, &mut server).is_empty(), "nothing new");

            // Riding: the passenger's rotation-only packet.
            game.riding = Some(99);
            game.set_local_rotation(12.0, 3.0).unwrap();
            game.remote_send_state(0);
            assert_eq!(sent_moves(game, &mut server), [("rot", 12.0, 3.0)]);
            game.riding = None;

            game.client.remote.as_mut().unwrap().logged_in = false;
            assert_eq!(
                game.set_local_rotation(0.0, 0.0),
                Err(LocalRotationError::NoWorld)
            );
        });
    });
}

/// `LocalPlayer.tickDeath` removes the player at deathTime 20: from then on nothing goes out, not
/// a changed input or look, until the respawn's new player (deathTime back at 0).
#[test]
#[ignore = "daily"]
fn a_player_dead_for_twenty_ticks_sends_nothing_until_it_respawns() {
    mod_platform::testing::with_mods(Vec::new().into(), || {
        with_client_game(|game| {
            let mut server = Connection::local(logged_client(game));
            server
                .reader
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            finish_login(game, &mut server);
            game.client.title.0 = None;
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
            game.remote_send_state(0);
            sent_packets(game, &mut server);

            game.client.screen = sunrise_gameplay_common::game::Screen::Dead;
            game.client.chat_ui.death_time = 19;
            game.yaw = 10.0;
            game.remote_send_state(1);
            assert!(sent_packets(game, &mut server) > 0, "still sent at 19");

            game.client.chat_ui.death_time = 20;
            for call in 0..25 {
                game.yaw = 20.0 + call as f32;
                game.remote_send_state(call as u8 & 1);
                assert_eq!(sent_packets(game, &mut server), 0, "call {call}");
            }

            // The respawn's new player starts its death clock again.
            game.client.screen = sunrise_gameplay_common::game::Screen::Playing;
            game.client.chat_ui.death_time = 0;
            game.yaw = 90.0;
            game.remote_send_state(1);
            assert!(sent_packets(game, &mut server) > 0, "sent after respawn");
        });
    });
}

/// `MultiPlayerGameMode.createPlayer`: the `LocalPlayer` a death's respawn or a reconfiguration's
/// login makes has sent no keys and no sprint (`Input.EMPTY`, `wasSprinting` false), like the new
/// server player, so keys still held go out again on its first tick. A dimension change's respawn
/// (`KEEP_ENTITY_DATA`) keeps the old player's last sent input.
#[test]
#[ignore = "daily"]
fn a_new_player_sends_its_held_keys_again_and_no_stale_sprint() {
    use sunrise_client::net::{Event, ingress};
    use sunrise_client_gameplay::remote::chunks::Received;
    use sunrise_protocol::packets::play::{self, Clientbound as C};

    // ServerboundPlayerInputPacket's flags: sneak is bit 5.
    const SNEAK: u8 = 32;
    let spawn = |dimension: &str| play::SpawnInfo {
        dimension_type: 0,
        dimension: dimension.into(),
        seed: 1,
        game_type: 0,
        previous_game_type: None,
        debug: false,
        flat: false,
        last_death: None,
        portal_cooldown: 0,
        sea_level: 63,
    };
    let login = |dimension: &str| {
        C::Login(play::Login {
            player_id: 7,
            hardcore: false,
            levels: vec!["minecraft:overworld".into(), "minecraft:the_nether".into()],
            max_players: 20,
            chunk_radius: 8,
            simulation_distance: 8,
            reduced_debug_info: false,
            show_death_screen: true,
            limited_crafting: false,
            spawn: spawn(dimension),
            online_mode: false,
            enforces_secure_chat: false,
        })
    };
    let position = |teleport| {
        C::PlayerPosition(play::PlayerPosition {
            teleport,
            position: [0.5, 65.0, 0.5],
            delta: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            relatives: 0,
        })
    };
    sunrise_gameplay_common::game::in_own_process(
        "cut_layout::game::client_side::remote_client::movement_input_tests::local_rotation_tests::a_new_player_sends_its_held_keys_again_and_no_stale_sprint",
        || {
            mod_platform::testing::with_mods(Vec::new().into(), || {
                with_client_game(|game| {
                    let mut server = Connection::local(logged_client(game));
                    server
                        .reader
                        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                        .unwrap();
                    finish_login(game, &mut server);
                    game.handle_packet(login("minecraft:overworld")).unwrap();
                    game.handle_packet(position(1)).unwrap();
                    game.client.title.0 = None;
                    game.player.as_mut().unwrap().sprinting = true;
                    game.remote_send_state(SNEAK);
                    assert_eq!(sent_input(game, &mut server), (vec![SNEAK], vec![true]));

                    // A death's respawn: sneak is still held, the new player doesn't sprint.
                    game.handle_packet(C::Respawn(play::Respawn {
                        spawn: spawn("minecraft:overworld"),
                        keep: 0,
                    }))
                    .unwrap();
                    game.handle_packet(position(2)).unwrap();
                    game.remote_send_state(SNEAK);
                    assert_eq!(sent_input(game, &mut server), (vec![SNEAK], vec![]));

                    // A sub-server switch: StartConfiguration, then the next server's login.
                    game.player.as_mut().unwrap().sprinting = true;
                    game.remote_send_state(SNEAK);
                    assert_eq!(sent_input(game, &mut server), (vec![], vec![true]));
                    let (tx, rx) = ingress::channel(1024, |_| 1).unwrap();
                    tx.send(Event::Reconfiguring(Box::new(
                        sunrise_client::net::PhaseChange::dummy_for_test(),
                    )))
                    .unwrap();
                    let (event, credit) = rx.try_recv_owned().unwrap().into_parts();
                    game.client
                        .remote
                        .as_mut()
                        .unwrap()
                        .backlog
                        .push_back(Received::Event(event, credit));
                    game.poll_remote_events(true).unwrap();
                    assert!(game.client.remote.as_ref().unwrap().reconfiguring.is_some());
                    game.handle_packet(login("minecraft:overworld")).unwrap();
                    game.handle_packet(position(3)).unwrap();
                    game.client.title.0 = None;
                    game.remote_send_state(SNEAK);
                    assert_eq!(sent_input(game, &mut server), (vec![SNEAK], vec![]));

                    // A dimension change keeps the last sent input: nothing new goes out.
                    game.handle_packet(C::Respawn(play::Respawn {
                        spawn: spawn("minecraft:the_nether"),
                        keep: 3,
                    }))
                    .unwrap();
                    game.handle_packet(position(4)).unwrap();
                    game.remote_send_state(SNEAK);
                    assert_eq!(sent_input(game, &mut server), (vec![], vec![]));
                });
            })
        },
    );
}
