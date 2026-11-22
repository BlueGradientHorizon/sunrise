//! Atomic play packet bundle dispatch against the recorded 26.3 witness.
use crate::game::{native_entity_refs::tests::with_client_game, remote::RemoteDimensionEntry};
use std::time::{Duration, Instant};
use sunrise_client::{
    level_loading::Reason, motion::PlayerMotion, multiplayer, net::Session, title::Mode,
};
use sunrise_core::{geometry::Vec3, nbt::Tag};
use sunrise_protocol::{
    connection::Pipe,
    entity_data::{PaintingHolder, PaintingVariant, Value, serializer},
    packets::{
        Empty, configuration as cfg,
        play::{self, Clientbound as C, Serverbound as S, SpawnInfo},
    },
};

fn witness_json(name: &str) -> serde_json::Value {
    let dir = std::env::var("SUNRISE_NETWORK_WITNESS_DIR")
        .expect("SUNRISE_NETWORK_WITNESS_DIR must be set (run via tools/test_network_parity.py)");
    let path = std::path::Path::new(&dir).join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

fn bundle_witness() -> serde_json::Value {
    witness_json("bundle.json")
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
#[ignore = "daily"]
fn genpvp_ping_rotation_and_keepalive_bundle_apply_atomically_in_order_26_3() {
    crate::game::in_own_process(
        "game::remote::raw_ingress::bundle_parity_tests::genpvp_ping_rotation_and_keepalive_bundle_apply_atomically_in_order_26_3",
        genpvp_ping_rotation_and_keepalive_bundle_body,
    );
}

fn genpvp_ping_rotation_and_keepalive_bundle_body() {
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
#[ignore = "daily"]
fn dependent_entity_bundle_never_splits_across_poll_or_tick_26_3() {
    crate::game::in_own_process(
        "game::remote::raw_ingress::bundle_parity_tests::dependent_entity_bundle_never_splits_across_poll_or_tick_26_3",
        dependent_entity_bundle_body,
    );
}

fn dependent_entity_bundle_body() {
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
            u32::from_str_radix(case["entity_eye_height_bits"].as_str().unwrap(), 16).unwrap();
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

#[test]
#[ignore = "daily"]
fn entity_data_validation_inline_painting_and_pre_login_bundle_errors_26_3() {
    crate::game::in_own_process(
        "game::remote::raw_ingress::bundle_parity_tests::entity_data_validation_inline_painting_and_pre_login_bundle_errors_26_3",
        entity_data_validation_inline_painting_and_pre_login_bundle_errors_body,
    );
}

fn entity_data_validation_inline_painting_and_pre_login_bundle_errors_body() {
    let bundle = bundle_witness();
    let entity_data = witness_json("entity_data.json");
    let assignment = &entity_data["metadata_assignment"];

    // 1. SetEntityData before Login disconnects with disconnect.packetError.
    with_client_game(|game| {
        game.title = None;
        let (client, _server) = Pipe::pair();
        game.connect_local(client, "MetadataBeforeLogin").unwrap();
        let (session, mut peer) = Session::play_loop_probe();
        let remote = game.remote.as_mut().unwrap();
        remote.session = session;
        remote.joined = true;
        remote.logged_in = false;
        peer.reader
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        peer.send(
            &C::SetEntityData(play::SetEntityData {
                id: 1,
                values: vec![(0, serializer::BYTE, Value::Byte(0x40))],
            })
            .encode(),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while game.remote.is_some() && Instant::now() < deadline {
            game.poll_remote().unwrap();
            std::thread::yield_now();
        }
        assert!(
            game.remote.is_none(),
            "SetEntityData before Login must disconnect"
        );
        let Some(Mode::Multiplayer(multiplayer::Screen::Disconnected { reason, .. })) =
            game.title.as_ref().map(|t| &t.mode)
        else {
            panic!("missing Disconnected screen for SetEntityData before Login");
        };
        assert_eq!(
            reason.translation_key(),
            assignment["before_login_disconnect_key"].as_str()
        );
    });

    // 2. Absent entity, repeated valid flags, valid prefix before invalid entry, bundle failure, and inline painting.
    with_client_game(|game| {
        game.title = None;
        let (client, _server) = Pipe::pair();
        game.connect_local(client, "MetadataValidation").unwrap();
        let spawn = SpawnInfo {
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
        };
        game.enter_remote_dimension(&spawn, RemoteDimensionEntry::Login, Reason::Other);
        let (session, mut peer) = Session::play_loop_probe();
        let remote = game.remote.as_mut().unwrap();
        remote.session = session;
        remote.joined = true;
        remote.logged_in = true;
        remote.entity_id = 1000;
        let entity_types = remote.builtin.get("minecraft:entity_type").unwrap();
        let interaction_kind = entity_types.id("minecraft:interaction").unwrap();
        let painting_kind = entity_types.id("minecraft:painting").unwrap();
        peer.reader
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();

        let wait_pong = |game: &mut crate::game::Game<'_>,
                         peer: &mut sunrise_protocol::connection::Connection,
                         ping_id: i32| {
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
        };

        // Absent entity is ignored and never enters the glowing set.
        assert_eq!(
            assignment["absent_entity_disconnect_key"].as_str(),
            Some("none")
        );
        peer.send(
            &C::SetEntityData(play::SetEntityData {
                id: 77_777,
                values: vec![(0, serializer::BYTE, Value::Byte(0x40))],
            })
            .encode(),
        )
        .unwrap();
        wait_pong(game, &mut peer, 1);
        assert!(game.remote.is_some());
        assert!(!game.remote.as_ref().unwrap().glowing.contains(&77_777));

        // Spawn an Interaction and a Painting.
        peer.send(
            &C::AddEntity(play::AddEntity {
                id: 1001,
                uuid: 1001,
                kind: interaction_kind,
                position: [0.5, 64.0, 0.5],
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
            &C::AddEntity(play::AddEntity {
                id: 2001,
                uuid: 2001,
                kind: painting_kind,
                position: [2.0, 65.0, 2.0],
                velocity: [0.0; 3],
                pitch: 0,
                yaw: 0,
                head_yaw: 0,
                data: 2,
            })
            .encode(),
        )
        .unwrap();

        // Repeated valid flag entries: last update wins.
        peer.send(
            &C::SetEntityData(play::SetEntityData {
                id: 1001,
                values: vec![
                    (0, serializer::BYTE, Value::Byte(0x40)),
                    (0, serializer::BYTE, Value::Byte(0x00)),
                ],
            })
            .encode(),
        )
        .unwrap();

        // Inline PaintingVariant updates asset and geometry (2x1).
        peer.send(
            &C::SetEntityData(play::SetEntityData {
                id: 2001,
                values: vec![(
                    9,
                    serializer::PAINTING_VARIANT,
                    Value::PaintingVariant(PaintingHolder::Direct(PaintingVariant {
                        width: 2,
                        height: 1,
                        asset_id: "minecraft:alban".into(),
                        title: Some(Tag::String("Alban".into())),
                        author: None,
                    })),
                )],
            })
            .encode(),
        )
        .unwrap();
        wait_pong(game, &mut peer, 2);
        assert_eq!(
            game.remote.as_ref().unwrap().glowing.contains(&1001),
            assignment["repeated_flags_last_wins"]["glowing_after"]
                .as_bool()
                .unwrap()
        );
        let painting = game.paintings.iter().find(|p| p.id == 2001).unwrap();
        assert_eq!(painting.asset.as_deref(), Some("alban"));
        assert_eq!((painting.width, painting.height), (2, 1));
        let (lo, hi) = painting.bounds();
        assert!((hi[0] - lo[0] - 2.0).abs() < 1e-6);
        assert!((hi[1] - lo[1] - 1.0).abs() < 1e-6);

        // Out-of-range accessor preserves valid prefix and raises PacketError.
        let err = game
            .apply_set_entity_data_packet(play::SetEntityData {
                id: 1001,
                values: vec![
                    (0, serializer::BYTE, Value::Byte(0x40)),
                    (99, serializer::BYTE, Value::Byte(1)),
                ],
            })
            .unwrap_err();
        assert!(err.is::<super::PacketError>());
        assert_eq!(
            game.remote.as_ref().unwrap().glowing.contains(&1001),
            assignment["out_of_range_preserves_prefix"]["glowing_after"]
                .as_bool()
                .unwrap()
        );
        game.remote.as_mut().unwrap().glowing.remove(&1001);

        // Mismatched serializer preserves valid prefix and raises PacketError.
        let err = game
            .apply_set_entity_data_packet(play::SetEntityData {
                id: 1001,
                values: vec![
                    (0, serializer::BYTE, Value::Byte(0x40)),
                    (0, serializer::INT, Value::Int(1)),
                ],
            })
            .unwrap_err();
        assert!(err.is::<super::PacketError>());
        assert_eq!(
            game.remote.as_ref().unwrap().glowing.contains(&1001),
            assignment["mismatched_serializer_preserves_prefix"]["glowing_after"]
                .as_bool()
                .unwrap()
        );

        // Inside a bundle, invalid entity metadata stops before a trailing Ping and disconnects.
        for packet in [
            C::BundleDelimiter(Empty),
            C::SetEntityData(play::SetEntityData {
                id: 1001,
                values: vec![
                    (0, serializer::BYTE, Value::Byte(0x40)),
                    (0, serializer::INT, Value::Int(1)),
                ],
            }),
            C::Ping(cfg::Ping { id: 55 }),
            C::BundleDelimiter(Empty),
        ] {
            peer.send(&packet.encode()).unwrap();
        }
        let mut saw_pong = false;
        let deadline = Instant::now() + Duration::from_secs(5);
        while game.remote.is_some() && Instant::now() < deadline {
            game.poll_remote().unwrap();
            while let Ok((id, body)) = peer.recv() {
                if matches!(S::decode(id, &body), Ok(S::Pong(p)) if p.id == 55) {
                    saw_pong = true;
                }
            }
            std::thread::yield_now();
        }
        assert!(
            game.remote.is_none(),
            "invalid SetEntityData in bundle must disconnect"
        );
        assert!(!saw_pong, "bundle must stop before trailing Ping(55)");
        let Some(Mode::Multiplayer(multiplayer::Screen::Disconnected { reason, .. })) =
            game.title.as_ref().map(|t| &t.mode)
        else {
            panic!("missing Disconnected screen after bundle metadata error");
        };
        assert_eq!(
            reason.translation_key(),
            assignment["mismatched_serializer_preserves_prefix"]["disconnect_key"].as_str()
        );
    });

    // 3. Pre-Login bundle [Disconnect("server kick"), PostEffects, Ping(88)] preserves first reason and stops.
    with_client_game(|game| {
        game.title = None;
        game.post_effects.clear();
        let (client, _server) = Pipe::pair();
        game.connect_local(client, "PreLoginDisconnectBundle")
            .unwrap();
        let (session, mut peer) = Session::play_loop_probe();
        let remote = game.remote.as_mut().unwrap();
        remote.session = session;
        remote.joined = true;
        remote.logged_in = false;
        peer.reader
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        for packet in [
            C::BundleDelimiter(Empty),
            C::Disconnect(cfg::Disconnect {
                reason: Tag::String("server kick".into()),
            }),
            C::PostEffects(play::PostEffects {
                effects: vec!["minecraft:invert".into()],
            }),
            C::Ping(cfg::Ping { id: 88 }),
            C::BundleDelimiter(Empty),
        ] {
            peer.send(&packet.encode()).unwrap();
        }
        let mut saw_pong = false;
        let deadline = Instant::now() + Duration::from_secs(5);
        while game.remote.is_some() && Instant::now() < deadline {
            game.poll_remote().unwrap();
            while let Ok((id, body)) = peer.recv() {
                if matches!(S::decode(id, &body), Ok(S::Pong(p)) if p.id == 88) {
                    saw_pong = true;
                }
            }
            std::thread::yield_now();
        }
        assert!(game.remote.is_none());
        assert!(!saw_pong, "bundle must stop before Ping(88)");
        assert!(game.post_effects.is_empty());
        let Some(Mode::Multiplayer(multiplayer::Screen::Disconnected { reason, .. })) =
            game.title.as_ref().map(|t| &t.mode)
        else {
            panic!("missing Disconnected screen");
        };
        assert_eq!(
            Some(reason.plain().as_str()),
            bundle["pre_login_disconnect_then_post_effects_bundle"]["disconnect_text"].as_str()
        );
    });

    // 4. Pre-Login bundle [PostEffects, Ping(89)] stops before Ping(89) with disconnect.packetError.
    with_client_game(|game| {
        game.title = None;
        game.post_effects.clear();
        let (client, _server) = Pipe::pair();
        game.connect_local(client, "PreLoginPostEffectsBundle")
            .unwrap();
        let (session, mut peer) = Session::play_loop_probe();
        let remote = game.remote.as_mut().unwrap();
        remote.session = session;
        remote.joined = true;
        remote.logged_in = false;
        peer.reader
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        for packet in [
            C::BundleDelimiter(Empty),
            C::PostEffects(play::PostEffects {
                effects: vec!["minecraft:invert".into()],
            }),
            C::Ping(cfg::Ping { id: 89 }),
            C::BundleDelimiter(Empty),
        ] {
            peer.send(&packet.encode()).unwrap();
        }
        let mut saw_pong = false;
        let deadline = Instant::now() + Duration::from_secs(5);
        while game.remote.is_some() && Instant::now() < deadline {
            game.poll_remote().unwrap();
            while let Ok((id, body)) = peer.recv() {
                if matches!(S::decode(id, &body), Ok(S::Pong(p)) if p.id == 89) {
                    saw_pong = true;
                }
            }
            std::thread::yield_now();
        }
        assert!(game.remote.is_none());
        assert!(!saw_pong, "bundle must stop before Ping(89)");
        assert!(game.post_effects.is_empty());
        let Some(Mode::Multiplayer(multiplayer::Screen::Disconnected { reason, .. })) =
            game.title.as_ref().map(|t| &t.mode)
        else {
            panic!("missing Disconnected screen");
        };
        assert_eq!(
            reason.translation_key(),
            bundle["pre_login_post_effects_then_ping_bundle"]["disconnect_key"].as_str()
        );
    });
}
