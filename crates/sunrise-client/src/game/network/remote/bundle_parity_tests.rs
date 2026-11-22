//! Atomic play packet bundle dispatch against the recorded 26.3 witness.
use crate::game::{native_entity_refs::tests::with_client_game, remote::RemoteDimensionEntry};
use std::time::{Duration, Instant};
use sunrise_client::{level_loading::Reason, motion::PlayerMotion, net::Session};
use sunrise_core::geometry::Vec3;
use sunrise_protocol::{
    connection::Pipe,
    entity_data::{Value, serializer},
    packets::{
        Empty, configuration as cfg,
        play::{self, Clientbound as C, Serverbound as S, SpawnInfo},
    },
};

fn bundle_witness() -> serde_json::Value {
    let dir = std::env::var("SUNRISE_NETWORK_WITNESS_DIR")
        .expect("SUNRISE_NETWORK_WITNESS_DIR must be set (run via tools/test_network_parity.py)");
    let path = std::path::Path::new(&dir).join("bundle.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

fn format_serverbound(packet: S) -> Option<String> {
    match packet {
        S::Pong(p) => Some(format!("pong:{}", p.id)),
        S::KeepAlive(k) => Some(format!("keep_alive:{}", k.id)),
        S::MovePlayerRot(m) => Some(format!(
            "move_player_rot:{:08x}:{:08x}",
            m.yaw.to_bits(),
            m.pitch.to_bits()
        )),
        _ => None,
    }
}

fn json_strings(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect()
}

#[test]
#[ignore = "run by tools/test_network_parity.py"]
fn genpvp_ping_rotation_and_keepalive_bundle_apply_atomically_in_order_26_3() {
    let _guard = super::PLAY_LOOP_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let witness = bundle_witness();
    let ping_rot = &witness["genpvp_ping_rotation_dispatch"];
    let expected_ping_rot = json_strings(&ping_rot["sent_after_process"]);
    let expected_yaw_bits =
        u32::from_str_radix(ping_rot["player_yRot_bits"].as_str().unwrap(), 16).unwrap();
    let expected_pitch_bits =
        u32::from_str_radix(ping_rot["player_xRot_bits"].as_str().unwrap(), 16).unwrap();
    let expected_keepalive =
        json_strings(&witness["keepalive_ordering"]["bundled"]["sent_after_process"]);

    with_client_game(|game| {
        let (client, _server) = Pipe::pair();
        game.connect_local(client, "BundleOrder").unwrap();
        let (session, mut peer) = Session::play_loop_probe();
        let remote = game.remote.as_mut().unwrap();
        remote.session = session;
        remote.joined = true;
        remote.logged_in = true;
        remote.entity_id = 83081;
        game.yaw = 0.0;
        game.pitch = 0.0;
        peer.reader
            .set_read_timeout(Some(Duration::from_millis(40)))
            .unwrap();

        // Open bundle without closing delimiter: neither packet may apply yet.
        for packet in [
            C::BundleDelimiter(Empty),
            C::Ping(cfg::Ping { id: -26 }),
            C::PlayerRotation(play::PlayerRotation {
                yaw: 22.5,
                relative_yaw: false,
                pitch: -11.25,
                relative_pitch: false,
            }),
        ] {
            peer.send(&packet.encode()).unwrap();
        }
        let mut early = Vec::new();
        let hold_until = Instant::now() + Duration::from_millis(80);
        while Instant::now() < hold_until {
            game.poll_remote().unwrap();
            while let Ok((id, body)) = peer.recv() {
                if let Some(tag) = format_serverbound(S::decode(id, &body).unwrap()) {
                    early.push(tag);
                }
            }
            std::thread::yield_now();
        }
        assert!(early.is_empty(), "unclosed bundle sent replies: {early:?}");
        assert_eq!(
            game.yaw.to_bits(),
            0.0f32.to_bits(),
            "unclosed bundle mutated player yaw"
        );

        // Close bundle: both packets apply together in wire order.
        peer.send(&C::BundleDelimiter(Empty).encode()).unwrap();
        let mut seen = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while seen.len() < expected_ping_rot.len() && Instant::now() < deadline {
            game.poll_remote().unwrap();
            while let Ok((id, body)) = peer.recv() {
                if let Some(tag) = format_serverbound(S::decode(id, &body).unwrap()) {
                    seen.push(tag);
                }
            }
            std::thread::yield_now();
        }
        assert_eq!(seen, expected_ping_rot);
        assert_eq!(game.yaw.to_bits(), expected_yaw_bits);
        assert_eq!(game.pitch.to_bits(), expected_pitch_bits);

        // Bundled KeepAlive stays in bundle order behind Ping and ahead of PlayerRotation.
        for packet in [
            C::BundleDelimiter(Empty),
            C::Ping(cfg::Ping { id: 7 }),
            C::KeepAlive(cfg::KeepAlive { id: 999 }),
            C::PlayerRotation(play::PlayerRotation {
                yaw: 90.0,
                relative_yaw: false,
                pitch: 12.5,
                relative_pitch: false,
            }),
            C::BundleDelimiter(Empty),
        ] {
            peer.send(&packet.encode()).unwrap();
        }
        assert!(
            peer.recv().is_err(),
            "bundled KeepAlive answered before game thread polled"
        );
        let mut keepalive_seen = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while keepalive_seen.len() < expected_keepalive.len() && Instant::now() < deadline {
            game.poll_remote().unwrap();
            while let Ok((id, body)) = peer.recv() {
                if let Some(tag) = format_serverbound(S::decode(id, &body).unwrap()) {
                    keepalive_seen.push(tag);
                }
            }
            std::thread::yield_now();
        }
        assert_eq!(keepalive_seen, expected_keepalive);
    });
}

#[test]
#[ignore = "run by tools/test_network_parity.py"]
fn dependent_entity_bundle_never_splits_across_poll_or_tick_26_3() {
    let _guard = super::PLAY_LOOP_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let witness = bundle_witness();
    let case = &witness["dependent_entity_bundle"];
    let entity_type = case["entity_type"].as_str().unwrap();
    let expected_exists_before = case["entity_exists_before_close"].as_bool().unwrap();
    let expected_cam_before = case["camera_entity_id_before_close"].as_i64().unwrap() as i32;
    let expected_exists_after = case["entity_exists_after_close"].as_bool().unwrap();
    let expected_width_bits =
        u32::from_str_radix(case["entity_width_bits"].as_str().unwrap(), 16).unwrap();
    let expected_height_bits =
        u32::from_str_radix(case["entity_height_bits"].as_str().unwrap(), 16).unwrap();
    let expected_cam_after = case["camera_entity_id_after_close"].as_i64().unwrap() as i32;

    with_client_game(|game| {
        let (client, _server) = Pipe::pair();
        game.connect_local(client, "DependentBundle").unwrap();
        let spawn = SpawnInfo {
            dimension_type: 0,
            dimension: "minecraft:overworld".into(),
            seed: 0,
            game_type: 3,
            previous_game_type: None,
            debug: false,
            flat: true,
            last_death: None,
            portal_cooldown: 0,
            sea_level: 63,
        };
        game.enter_remote_dimension(&spawn, RemoteDimensionEntry::Login, Reason::Other);
        game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
        let (session, mut peer) = Session::play_loop_probe();
        let remote = game.remote.as_mut().unwrap();
        remote.session = session;
        remote.joined = true;
        remote.logged_in = true;
        remote.entity_id = expected_cam_before;
        let entity_types = remote.builtin.get("minecraft:entity_type").unwrap();
        let expected_eye_height_bits =
            u32::from_str_radix(dep["entity_eye_height_bits"].as_str().unwrap(), 16).unwrap();
        let interaction_kind = entity_types.id(entity_type).unwrap();
        let creeper_kind = entity_types.id("minecraft:creeper").unwrap();

        // First half of bundle: AddEntity without closing delimiter.
        peer.send(&C::BundleDelimiter(Empty).encode()).unwrap();
        peer.send(
            &C::AddEntity(play::AddEntity {
                id: expected_cam_after,
                uuid: expected_cam_after as u128,
                kind: interaction_kind,
                position: [0.5, 300.0, 0.5],
                velocity: [0.0; 3],
                pitch: 0,
                yaw: 0,
                head_yaw: 0,
                data: 0,
            })
            .encode(),
        )
        .unwrap();
        let hold_until = Instant::now() + Duration::from_millis(80);
        while Instant::now() < hold_until {
            game.poll_remote().unwrap();
            assert_eq!(
                game.interactions.contains_key(&expected_cam_after),
                expected_exists_before,
                "unclosed bundle spawned entity before closing delimiter"
            );
            assert_eq!(
                game.remote_camera_id()
                    .map(|id| id as i32)
                    .unwrap_or(expected_cam_before),
                expected_cam_before
            );
            std::thread::yield_now();
        }

        // Second half: SetEntityData + SetCamera + closing delimiter apply atomically on the same Interaction.
        peer.send(
            &C::SetEntityData(play::SetEntityData {
                id: expected_cam_after,
                values: vec![
                    (8, serializer::FLOAT, Value::Float(0.1)),
                    (9, serializer::FLOAT, Value::Float(1.9059)),
                ],
            })
            .encode(),
        )
        .unwrap();
        peer.send(
            &C::SetCamera(play::SetCamera {
                camera_id: expected_cam_after,
            })
            .encode(),
        )
        .unwrap();
        peer.send(&C::BundleDelimiter(Empty).encode()).unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut applied = false;
        while Instant::now() < deadline {
            game.poll_remote().unwrap();
            let exists = game.interactions.contains_key(&expected_cam_after);
            let cam = game.remote_camera_id().map(|id| id as i32);
            if exists || cam.is_some() {
                assert!(exists && expected_exists_after);
                let interaction = game.interactions.get(&expected_cam_after).unwrap();
                assert_eq!(interaction.width.to_bits(), expected_width_bits);
                assert_eq!(interaction.height.to_bits(), expected_height_bits);
                assert_eq!(cam, Some(expected_cam_after));
                let view = game
                    .spectated_camera_snapshot()
                    .expect("Interaction camera view");
                assert_eq!(view.eye_height.to_bits(), expected_eye_height_bits);
                applied = true;
                break;
            }
            std::thread::yield_now();
        }
        assert!(
            applied,
            "closed interaction AddEntity+SetEntityData+SetCamera bundle never applied"
        );

        // Switching camera from a creeper (with creeper shader) to the resident Interaction clears the shader.
        let creeper_id = expected_cam_after + 1;
        peer.send(
            &C::AddEntity(play::AddEntity {
                id: creeper_id,
                uuid: creeper_id as u128,
                kind: creeper_kind,
                position: [2.5, 300.0, 2.5],
                velocity: [0.0; 3],
                pitch: 0,
                yaw: 0,
                head_yaw: 0,
                data: 0,
            })
            .encode(),
        )
        .unwrap();
        peer.send(
            &C::SetCamera(play::SetCamera {
                camera_id: creeper_id,
            })
            .encode(),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            game.poll_remote().unwrap();
            if game.remote_camera_id() == Some(creeper_id as u32) {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(game.remote_camera_id(), Some(creeper_id as u32));
        assert_eq!(game.spectated_post_effect, Some("minecraft:creeper"));
        assert!(game.spectated_post_effect_active);

        peer.send(
            &C::SetCamera(play::SetCamera {
                camera_id: expected_cam_after,
            })
            .encode(),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            game.poll_remote().unwrap();
            if game.remote_camera_id() == Some(expected_cam_after as u32) {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(game.remote_camera_id(), Some(expected_cam_after as u32));
        assert_eq!(game.spectated_post_effect, None);
        assert!(!game.spectated_post_effect_active);
    });
}
