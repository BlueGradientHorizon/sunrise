use crate::game::{
    Game, native_entity_refs::tests::with_client_game, native_lifecycle::NativeLifecycle,
};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use sunrise_client::{
    camera::CameraType,
    chunk::{Chunk, Light},
    debug_entries::Status,
    debug_overlay::Displayer,
    effects::EffectInstance,
    environment::Sky,
    items::ItemStack,
    level_loading::Reason,
    motion::PlayerMotion,
    options_file::V as OptV,
    screen_effects::Blend,
};
use sunrise_core::geometry::Vec3;
use sunrise_protocol::{
    buf::pack_degrees,
    connection::Pipe,
    entity_data::{Value as EntityDataValue, serializer},
    item::Stack as WireStack,
    packets::play::{self, Clientbound as C, SpawnInfo},
};

const PARITY_WITNESS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/witness/26.3/camera_parity/witness.json"
));

fn parity_witness() -> Value {
    serde_json::from_str(PARITY_WITNESS).expect("valid 26.3 camera parity witness")
}

fn menu_witness_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/witness/26.3/genpvp_menu")
}

fn hex_f32(v: &Value) -> u32 {
    u32::from_str_radix(v.as_str().unwrap(), 16).unwrap()
}

fn hex_f64(v: &Value) -> u64 {
    u64::from_str_radix(v.as_str().unwrap(), 16).unwrap()
}

fn vec3_from_bits(v: &Value) -> [f64; 3] {
    [
        f64::from_bits(hex_f64(&v[0])),
        f64::from_bits(hex_f64(&v[1])),
        f64::from_bits(hex_f64(&v[2])),
    ]
}

fn vec3_bits_expected(v: &Value) -> [u64; 3] {
    [hex_f64(&v[0]), hex_f64(&v[1]), hex_f64(&v[2])]
}

fn entity_type_id(game: &Game<'_>, kind: &str) -> i32 {
    game.remote
        .as_ref()
        .unwrap()
        .builtin
        .get("minecraft:entity_type")
        .unwrap()
        .id(kind)
        .unwrap_or_else(|| panic!("missing entity type {kind}"))
}

#[must_use]
fn setup_remote_overworld(
    game: &mut Game<'_>,
    player_id: i32,
    player_pos: [f64; 3],
    flat: bool,
) -> Pipe {
    game.native_lifecycle = NativeLifecycle::default();
    let (client, server) = Pipe::pair();
    game.connect_local(client, "CameraParityTest").unwrap();
    let spawn = SpawnInfo {
        dimension_type: 0,
        dimension: "minecraft:overworld".into(),
        seed: 0,
        game_type: 0,
        previous_game_type: None,
        debug: false,
        flat,
        last_death: None,
        portal_cooldown: 0,
        sea_level: 63,
    };
    game.enter_remote_dimension(&spawn, false, Reason::Other);
    let remote = game.remote.as_mut().unwrap();
    remote.logged_in = true;
    remote.entity_id = player_id;
    remote.view_distance = 8;
    remote.level_load.finish();
    game.player = Some(
        PlayerMotion::new(
            game.motion,
            Vec3::new(player_pos[0], player_pos[1], player_pos[2]),
        )
        .unwrap(),
    );
    game.camera_type = CameraType::FirstPerson;
    game.settings.fov = 70.0;
    game.settings.render_distance = 8;
    server
}

fn add_entity(game: &mut Game<'_>, id: i32, kind: &str, position: [f64; 3], yaw: f32, pitch: f32) {
    let kind_id = entity_type_id(game, kind);
    let y = pack_degrees(yaw);
    let p = pack_degrees(pitch);
    game.handle_packet(C::AddEntity(play::AddEntity {
        id,
        uuid: id as u128,
        kind: kind_id,
        position,
        velocity: [0.0; 3],
        pitch: p,
        yaw: y,
        head_yaw: y,
        data: 0,
    }))
    .unwrap();
}

fn sync_interaction_size(game: &mut Game<'_>, id: i32, width: f32, height: f32) {
    game.handle_packet(C::SetEntityData(play::SetEntityData {
        id,
        values: vec![
            (8, 3, EntityDataValue::Float(width)),
            (9, 3, EntityDataValue::Float(height)),
        ],
    }))
    .unwrap();
}

fn make_chunk(cx: i32, cz: i32, biome_id: u16, uniform_light: u8) -> Chunk {
    let mut chunk = Chunk::empty(cx, cz);
    chunk.lit = true;
    for qy in 0..96 {
        for qz in 0..4 {
            for qx in 0..4 {
                chunk.set_biome_cell(qx, qy, qz, biome_id);
            }
        }
    }
    for section in &mut chunk.sections {
        Arc::make_mut(section).light = Light::Uniform(uniform_light);
    }
    chunk
}

/// Items 1 & 12 against the real vanilla 26.3 genpvp menu witness (`settled.json` & `ticks.jsonl`).
#[test]
#[ignore = "daily"]
fn genpvp_menu_interaction_camera_matches_26_3_witness() {
    let dir = menu_witness_dir();
    let settled: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("settled.json"))
            .expect("tools/witness/26.3/genpvp_menu/settled.json"),
    )
    .unwrap();
    let ticks_raw = std::fs::read_to_string(dir.join("ticks.jsonl"))
        .expect("tools/witness/26.3/genpvp_menu/ticks.jsonl");
    let ticks: Vec<Value> = ticks_raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let player_id = settled["player"]["id"].as_i64().unwrap() as i32;
    let player_pos = vec3_from_bits(&settled["player"]["pos_bits"]);
    let cam_id = settled["camera"]["entity_id"].as_i64().unwrap() as i32;
    let cam_type = settled["camera"]["entity_type"].as_str().unwrap();
    let cam_entity_pos = vec3_from_bits(&settled["camera"]["entity_pos_bits"]);
    let partial = f32::from_bits(hex_f32(&settled["camera"]["partial_tick_bits"]));

    with_client_game(|game| {
        let _server = setup_remote_overworld(game, player_id, player_pos, false);
        add_entity(game, cam_id, cam_type, cam_entity_pos, 0.0, 0.0);
        sync_interaction_size(game, cam_id, 0.1, 1.9059);
        game.handle_packet(C::SetCamera(play::SetCamera { camera_id: cam_id }))
            .unwrap();

        let snap = game
            .spectated_camera_snapshot()
            .expect("interaction camera entity must resolve");
        assert_eq!(
            snap.position.map(f64::to_bits),
            vec3_bits_expected(&settled["camera"]["entity_pos_bits"]),
            "camera entity position bits"
        );
        assert_eq!(
            snap.eye_height.to_bits(),
            hex_f32(&settled["camera"]["entity_eye_height_bits"]),
            "interaction eye height bits"
        );

        let attached_ticks: Vec<&Value> = ticks
            .iter()
            .filter(|row| row["camera_entity_id"].as_i64() == Some(i64::from(cam_id)))
            .collect();
        assert!(
            !attached_ticks.is_empty(),
            "ticks.jsonl must record camera switch"
        );
        for row in [
            attached_ticks[0],
            attached_ticks[attached_ticks.len() / 2],
            *attached_ticks.last().unwrap(),
        ] {
            assert_eq!(
                snap.position.map(f64::to_bits),
                vec3_bits_expected(&row["camera_entity_pos_bits"]),
                "tick {} camera_entity_pos_bits",
                row["tick"]
            );
        }

        game.view = Default::default();
        for _ in 0..40 {
            game.tick_fov();
        }
        let (cam_pos, yaw, pitch) = game.camera_view(partial).expect("camera view");
        assert_eq!(
            cam_pos.to_array().map(f64::to_bits),
            vec3_bits_expected(&settled["camera"]["camera_pos_bits"]),
            "settled camera_pos_bits"
        );
        assert_eq!(
            yaw.to_bits(),
            hex_f32(&settled["camera"]["camera_yRot_bits"]),
            "settled camera_yRot_bits"
        );
        assert_eq!(
            pitch.to_bits(),
            hex_f32(&settled["camera"]["camera_xRot_bits"]),
            "settled camera_xRot_bits"
        );
        assert_eq!(
            game.view.detached,
            settled["camera"]["detached"].as_bool().unwrap()
        );
        assert_eq!(
            game.fov_degrees(partial, cam_pos.to_array()).to_bits(),
            hex_f32(&settled["frame"]["fov_bits"]),
            "settled frame.fov_bits"
        );
    });
}

/// Item 1: every tracked entity kind resolves through the single camera resolver (26.3 oracle).
#[test]
#[ignore = "daily"]
fn any_tracked_entity_resolves_as_camera_26_3() {
    let witness = parity_witness();
    let records = witness["entity_cameras"]["records"].as_array().unwrap();
    with_client_game(|game| {
        let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
        let mut mismatches = Vec::new();
        for (idx, row) in records.iter().enumerate() {
            let label = row["label"].as_str().unwrap();
            let kind = row["type"].as_str().unwrap();
            let synced = row["synced_genpvp"].as_bool().unwrap();
            let id = 100 + idx as i32;
            add_entity(game, id, kind, [12.25, 70.5, -4.75], 45.0, 15.0);
            if synced {
                sync_interaction_size(game, id, 0.1, 1.9059);
            }
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: id }))
                .unwrap();

            let Some(snap) = game.spectated_camera_snapshot() else {
                mismatches.push(format!("{label}: spectated_camera_snapshot returned None"));
                continue;
            };
            if snap.eye_height.to_bits() != hex_f32(&row["eye_height_bits"]) {
                mismatches.push(format!(
                    "{label}: eye_height {:08x} != vanilla {}",
                    snap.eye_height.to_bits(),
                    row["eye_height_bits"]
                ));
            }
            if snap.living.is_some() != row["is_living"].as_bool().unwrap() {
                mismatches.push(format!(
                    "{label}: is_living {} != vanilla {}",
                    snap.living.is_some(),
                    row["is_living"]
                ));
            }

            game.view = Default::default();
            game.tick_fov();
            let (pos1, y1, p1) = game.camera_view(1.0).expect("camera view");
            if pos1.to_array().map(f64::to_bits)
                != vec3_bits_expected(&row["camera_pos_after_1_tick_bits"])
            {
                mismatches.push(format!(
                    "{label}: pos1 {:?} != vanilla {}",
                    pos1.to_array().map(|v| format!("{:016x}", v.to_bits())),
                    row["camera_pos_after_1_tick_bits"]
                ));
            }
            if y1.to_bits() != hex_f32(&row["camera_yRot_bits"])
                || p1.to_bits() != hex_f32(&row["camera_xRot_bits"])
            {
                mismatches.push(format!(
                    "{label}: rot ({:08x}, {:08x}) != vanilla ({}, {})",
                    y1.to_bits(),
                    p1.to_bits(),
                    row["camera_yRot_bits"],
                    row["camera_xRot_bits"]
                ));
            }
            let fw = game.view.forwards.map(f32::to_bits);
            let want_fw = [
                hex_f32(&row["camera_forwards_bits"][0]),
                hex_f32(&row["camera_forwards_bits"][1]),
                hex_f32(&row["camera_forwards_bits"][2]),
            ];
            if fw != want_fw {
                mismatches.push(format!("{label}: forwards {fw:08x?} != {want_fw:08x?}"));
            }

            for _ in 0..40 {
                game.tick_fov();
            }
            let (pos_settled, _, _) = game.camera_view(1.0).expect("settled camera view");
            if pos_settled.to_array().map(f64::to_bits)
                != vec3_bits_expected(&row["camera_pos_settled_bits"])
            {
                mismatches.push(format!(
                    "{label}: settled pos {:?} != vanilla {}",
                    pos_settled
                        .to_array()
                        .map(|v| format!("{:016x}", v.to_bits())),
                    row["camera_pos_settled_bits"]
                ));
            }
        }
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    });
}

/// Item 12: Interaction eye height, rotation, move/teleport interpolation, and far snap (26.3).
#[test]
#[ignore = "daily"]
fn interaction_camera_eye_rotation_move_and_teleport_26_3() {
    let witness = parity_witness();
    let life = &witness["interaction_lifecycle"];
    with_client_game(|game| {
        let _server = setup_remote_overworld(game, 1, [0.0, 64.0, 0.0], true);

        // 1. Unsynced spawn keeps eye_height == 0.0 and stores spawn rotation.
        add_entity(
            game,
            200,
            "minecraft:interaction",
            [0.5, 300.0, 0.5],
            90.0,
            30.0,
        );
        game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 200 }))
            .unwrap();
        let snap = game.spectated_camera_snapshot().expect("interaction 200");
        assert_eq!(
            snap.eye_height.to_bits(),
            hex_f32(&life["unsynced_spawn"]["eye_height_bits"])
        );
        assert_eq!(
            snap.y_rot.to_bits(),
            hex_f32(&life["unsynced_spawn"]["yRot_bits"])
        );
        assert_eq!(
            snap.x_rot.to_bits(),
            hex_f32(&life["unsynced_spawn"]["xRot_bits"])
        );

        // 2. Explicit default (1.0, 1.0) refreshes dimensions to eye_height = 0.85.
        sync_interaction_size(game, 200, 1.0, 1.0);
        let snap = game.spectated_camera_snapshot().unwrap();
        assert_eq!(
            snap.eye_height.to_bits(),
            hex_f32(&life["sync_default_1x1"]["eye_height_bits"])
        );

        // 3. DATA_POSE (6) alone refreshes dimensions on a fresh unsynced Interaction.
        add_entity(
            game,
            201,
            "minecraft:interaction",
            [0.5, 300.0, 0.5],
            0.0,
            0.0,
        );
        game.handle_packet(C::SetEntityData(play::SetEntityData {
            id: 201,
            values: vec![(6, 21, EntityDataValue::Id(0))],
        }))
        .unwrap();
        game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 201 }))
            .unwrap();
        let snap = game.spectated_camera_snapshot().unwrap();
        assert_eq!(
            snap.eye_height.to_bits(),
            hex_f32(&life["sync_pose_only"]["eye_height_bits"])
        );

        // 4. MoveEntityPosRot, relative TeleportEntity (Y_ROT|X_ROT), and far TeleportEntity snap.
        add_entity(
            game,
            210,
            "minecraft:interaction",
            [10.0, 64.0, -5.0],
            0.0,
            0.0,
        );
        sync_interaction_size(game, 210, 0.1, 1.9059);
        game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 210 }))
            .unwrap();
        game.view = Default::default();
        for _ in 0..40 {
            game.tick_fov();
        }

        // Tick 1: snapshot old pose, then MoveEntityPosRot (+1.0, +0.5, -0.25), yaw 45, pitch 15.
        game.tick_remote_entities(true);
        game.handle_packet(C::MoveEntityPosRot(play::MoveEntityPosRot(
            play::MoveEntity {
                id: 210,
                delta: Some(play::VecDelta::Linear([4096, 2048, -1024])),
                rotation: Some((pack_degrees(45.0), pack_degrees(15.0))),
                on_ground: false,
            },
        )))
        .unwrap();
        for frame in life["after_move_pos_rot_frames"].as_array().unwrap() {
            let pt = f32::from_bits(hex_f32(&frame["partial_bits"]));
            let (pos, yaw, pitch) = game.camera_view(pt).unwrap();
            assert_eq!(
                pos.to_array().map(f64::to_bits),
                vec3_bits_expected(&frame["camera_pos_bits"]),
                "move pos at partial {pt}"
            );
            assert_eq!(
                yaw.to_bits(),
                hex_f32(&frame["camera_yRot_bits"]),
                "move yaw at partial {pt}"
            );
            assert_eq!(
                pitch.to_bits(),
                hex_f32(&frame["camera_xRot_bits"]),
                "move pitch at partial {pt}"
            );
        }

        // Tick 2: snapshot old pose, then relative TeleportEntity (+30 yaw, -10 pitch).
        game.tick_remote_entities(true);
        game.handle_packet(C::TeleportEntity(play::TeleportEntity {
            id: 210,
            position: [12.0, 65.0, -4.0],
            delta: [0.0; 3],
            yaw: 30.0,
            pitch: -10.0,
            relatives: play::relative::Y_ROT | play::relative::X_ROT,
            on_ground: false,
        }))
        .unwrap();
        for frame in life["after_relative_teleport_frames"].as_array().unwrap() {
            let pt = f32::from_bits(hex_f32(&frame["partial_bits"]));
            let (pos, yaw, pitch) = game.camera_view(pt).unwrap();
            assert_eq!(
                pos.to_array().map(f64::to_bits),
                vec3_bits_expected(&frame["camera_pos_bits"]),
                "rel tp pos at partial {pt}"
            );
            assert_eq!(
                yaw.to_bits(),
                hex_f32(&frame["camera_yRot_bits"]),
                "rel tp yaw at partial {pt}"
            );
            assert_eq!(
                pitch.to_bits(),
                hex_f32(&frame["camera_xRot_bits"]),
                "rel tp pitch at partial {pt}"
            );
        }

        // Tick 3: snapshot old pose, then far TeleportEntity (> 64 blocks) snaps old pose too.
        game.tick_remote_entities(true);
        game.handle_packet(C::TeleportEntity(play::TeleportEntity {
            id: 210,
            position: [100.0, 65.0, -4.0],
            delta: [0.0; 3],
            yaw: 180.0,
            pitch: -20.0,
            relatives: 0,
            on_ground: false,
        }))
        .unwrap();
        for frame in life["after_far_teleport_frames"].as_array().unwrap() {
            let pt = f32::from_bits(hex_f32(&frame["partial_bits"]));
            let (pos, yaw, pitch) = game.camera_view(pt).unwrap();
            assert_eq!(
                pos.to_array().map(f64::to_bits),
                vec3_bits_expected(&frame["camera_pos_bits"]),
                "far tp pos at partial {pt}"
            );
            assert_eq!(
                yaw.to_bits(),
                hex_f32(&frame["camera_yRot_bits"]),
                "far tp yaw at partial {pt}"
            );
            assert_eq!(
                pitch.to_bits(),
                hex_f32(&frame["camera_xRot_bits"]),
                "far tp pitch at partial {pt}"
            );
        }
    });
}

/// Item 12: RemoveEntities retains the detached camera entity and post effect until SetCamera (26.3).
#[test]
#[ignore = "daily"]
fn removing_camera_entity_keeps_detached_camera_26_3() {
    let witness = parity_witness();
    for kind in ["minecraft:interaction", "minecraft:creeper"] {
        let case = &witness["camera_removal"][kind];
        with_client_game(|game| {
            let _server = setup_remote_overworld(game, 7, [0.5, 64.0, 0.5], true);
            add_entity(game, 31, kind, [10.5, 300.0, 12.5], 45.0, 10.0);
            if kind == "minecraft:interaction" {
                sync_interaction_size(game, 31, 0.1, 1.9059);
            }
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 31 }))
                .unwrap();
            game.view = Default::default();
            for _ in 0..40 {
                game.tick_fov();
            }
            let (pos_before, _, _) = game.camera_view(1.0).unwrap();
            assert_eq!(
                game.controls_camera(),
                case["after_set_camera"]["is_controlled_camera"]
                    .as_bool()
                    .unwrap(),
                "{kind} before remove"
            );
            assert_eq!(
                game.spectated_post_effect,
                case["after_set_camera"]["post_effect"].as_str(),
                "{kind} post effect before remove"
            );
            assert_eq!(
                pos_before.to_array().map(f64::to_bits),
                vec3_bits_expected(&case["after_set_camera"]["camera_pos_bits"]),
                "{kind} pos before remove"
            );

            game.handle_packet(C::RemoveEntities(play::RemoveEntities { ids: vec![31] }))
                .unwrap();
            game.tick_fov();
            let (pos_after, y_after, p_after) = game.camera_view(1.0).unwrap();
            assert_eq!(
                game.controls_camera(),
                case["after_remove_entities"]["is_controlled_camera"]
                    .as_bool()
                    .unwrap(),
                "{kind} must stay detached after removal"
            );
            assert_eq!(
                game.spectated_post_effect,
                case["after_remove_entities"]["post_effect"].as_str(),
                "{kind} post effect must persist after removal"
            );
            assert_eq!(
                pos_after.to_array().map(f64::to_bits),
                vec3_bits_expected(&case["after_remove_entities"]["camera_pos_bits"]),
                "{kind} camera pos must remain at removed entity"
            );
            assert_eq!(
                y_after.to_bits(),
                hex_f32(&case["after_remove_entities"]["camera_yRot_bits"]),
                "{kind} camera yaw after removal"
            );
            assert_eq!(
                p_after.to_bits(),
                hex_f32(&case["after_remove_entities"]["camera_xRot_bits"]),
                "{kind} camera pitch after removal"
            );

            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 7 }))
                .unwrap();
            assert_eq!(
                game.controls_camera(),
                case["after_set_camera_player"]["is_controlled_camera"]
                    .as_bool()
                    .unwrap(),
                "{kind} controls_camera after restore"
            );
            assert_eq!(
                game.spectated_post_effect,
                case["after_set_camera_player"]["post_effect"].as_str(),
                "{kind} post effect after restore"
            );
        });
    }
}

/// Item 6: bob, fog, dark disc, vignette, border warning, and F1 hide_gui follow 26.3 camera rules.
#[test]
#[ignore = "daily"]
fn camera_consumers_bob_fog_dark_disc_and_vignette_26_3() {
    let witness = parity_witness();
    let consumers = &witness["camera_consumers"];
    with_client_game(|game| {
        let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], false);

        // Give the local player hurt + walk bob, Spectator mode, and fog-altering potions.
        {
            let p = game.player.as_mut().unwrap();
            p.health.hurt_animation_time = 6;
            p.health.hurt.hurt_duration = 10;
            p.health.hurt_dir = 45.0;
        }
        game.avatar.walk_dist = 1.75;
        game.avatar.walk_dist_o = 1.75;
        game.avatar.bob = 0.08;
        game.avatar.bob_o = 0.04;
        game.settings.view_bobbing = true;
        game.set_mode_id(3);
        let profile = game.remote.as_ref().unwrap().profile;
        game.handle_packet(C::PlayerInfoUpdate(play::PlayerInfoUpdate {
            actions: 0x01 | 0x04 | 0x08,
            entries: vec![play::PlayerInfo {
                uuid: profile,
                name: Some("CameraParityTest".into()),
                game_mode: Some(3),
                listed: Some(true),
                ..play::PlayerInfo::default()
            }],
        }))
        .unwrap();
        game.potions
            .effects
            .add(EffectInstance::new("blindness", 100, 0));
        game.potions
            .effects
            .add(EffectInstance::new("fire_resistance", 100, 0));
        game.potions
            .effects
            .add(EffectInstance::new("night_vision", 300, 0));
        game.fog.water_vision_time = 0;

        add_entity(
            game,
            2,
            "minecraft:interaction",
            [0.5, 300.0, 0.5],
            0.0,
            0.0,
        );
        sync_interaction_size(game, 2, 0.1, 1.9059);
        game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 2 }))
            .unwrap();
        game.view = Default::default();
        for _ in 0..40 {
            game.tick_fov();
        }
        let (cam, _, _) = game.camera_view(1.0).unwrap();

        // 1. Bob: non-living Interaction camera produces identity matrix even when player is hurt/walking.
        let bob_cols: Vec<u32> = game
            .bob_matrix(0.5)
            .to_cols_array()
            .into_iter()
            .map(f32::to_bits)
            .collect();
        let want_bob: Vec<u32> = consumers["bob"]["interaction"]["matrix_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(hex_f32)
            .collect();
        assert_eq!(bob_cols, want_bob, "interaction camera must have no bob");

        // 2. Fog: Interaction is neither Spectator nor LivingEntity, and is not LocalPlayer.
        game.world
            .chunks
            .insert((0, 0), Arc::new(make_chunk(0, 0, 0, 0xF0)));
        let sky = Sky {
            fog_start: 16.0,
            fog_end: 128.0,
            sky_fog_end: 128.0,
            water_fog_start: -8.0,
            water_fog_end: 96.0,
            cloud_fog_end: 2048.0,
            ..Sky::default()
        };
        let want_fog = &consumers["fog"]["interaction"];

        let lava = game
            .pack
            .assets
            .registry
            .default_state("minecraft:lava")
            .unwrap();
        game.world.set(0, 301, 0, lava).unwrap();
        let lava_fog = game.frame_fog(cam, &sky, 0, [0.0; 3]);
        assert_eq!(
            lava_fog.environmental_start.to_bits(),
            hex_f32(&want_fog["lava_start_bits"]),
            "interaction camera lava fog start"
        );
        assert_eq!(
            lava_fog.environmental_end.to_bits(),
            hex_f32(&want_fog["lava_end_bits"]),
            "interaction camera lava fog end"
        );

        let snow = game
            .pack
            .assets
            .registry
            .default_state("minecraft:powder_snow")
            .unwrap();
        game.world.set(0, 301, 0, snow).unwrap();
        let snow_fog = game.frame_fog(cam, &sky, 0, [0.0; 3]);
        assert_eq!(
            snow_fog.environmental_start.to_bits(),
            hex_f32(&want_fog["powder_snow_start_bits"]),
            "interaction camera powder snow fog start"
        );
        assert_eq!(
            snow_fog.environmental_end.to_bits(),
            hex_f32(&want_fog["powder_snow_end_bits"]),
            "interaction camera powder snow fog end"
        );

        let water = game
            .pack
            .assets
            .registry
            .default_state("minecraft:water")
            .unwrap();
        game.world.set(0, 301, 0, water).unwrap();
        let water_fog = game.frame_fog(cam, &sky, 0, [0.1, 0.2, 0.6]);
        assert_eq!(
            water_fog.environmental_end.to_bits(),
            hex_f32(&want_fog["water_end_bits"]),
            "non-local camera must not apply LocalPlayer waterVision scaling"
        );

        let air = game
            .pack
            .assets
            .registry
            .default_state("minecraft:air")
            .unwrap();
        game.world.set(0, 301, 0, air).unwrap();
        let air_fog = game.frame_fog(cam, &sky, 0, [0.0; 3]);
        assert!(
            air_fog.environmental_end > 10.0,
            "local player blindness must not clamp Interaction atmospheric fog: {}",
            air_fog.environmental_end
        );

        // 3. Dark disc reads local player eye Y, NOT camera Y.
        game.player.as_mut().unwrap().position = Vec3::new(0.5, 50.0, 0.5);
        assert_eq!(
            game.dark_disc(cam),
            consumers["dark_disc"]["player_below_horizon_dry"]
                .as_bool()
                .unwrap(),
            "player below horizon with camera at y=301.62 must still render dark disc"
        );
        game.player.as_mut().unwrap().position = Vec3::new(0.5, 100.0, 0.5);
        assert_eq!(
            game.dark_disc(glam::DVec3::new(0.5, 50.0, 0.5)),
            consumers["dark_disc"]["player_above_horizon_dry"]
                .as_bool()
                .unwrap(),
            "player above horizon with camera at y=50 must NOT render dark disc"
        );

        // 4. Vignette brightness & border warning follow camera entity, and F1 hides GUI overlays.
        game.player.as_mut().unwrap().position = Vec3::new(0.5, 64.0, 0.5);
        game.world.day_time = 6000;
        game.world
            .chunks
            .insert((5, 0), Arc::new(make_chunk(5, 0, 0, 0x00)));
        add_entity(
            game,
            3,
            "minecraft:interaction",
            [95.5, 300.0, 0.5],
            0.0,
            0.0,
        );
        sync_interaction_size(game, 3, 0.1, 1.9059);
        game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 3 }))
            .unwrap();
        game.screen_fx.vignette_brightness = 0.0;
        game.tick_vignette();
        assert_eq!(
            game.screen_fx.vignette_brightness.to_bits(),
            hex_f32(&consumers["vignette"]["interaction_vignette_after_1_tick_bits"]),
            "vignette must sample light at camera entity eye"
        );

        game.border.set_center([0.0, 0.0]);
        game.border.set_size(200.0);
        game.border.warning_distance = 10;
        game.settings.vignette = true;
        game.ui.hide_gui = false;
        let hand_proj = sunrise_client::camera::hand_projection(70.0, 1280.0, 800.0, 512.0);
        let quads_shown = game.screen_quads(1.0, [1280, 800], 1.0, hand_proj);
        let vignette_quad = quads_shown
            .iter()
            .find(|q| q.blend == Blend::Vignette)
            .expect("vignette quad expected when HUD visible");
        let color = vignette_quad.vertices[0].color;
        assert!(
            color[1] > color[0] + 0.1,
            "border warning at camera x=95.5 must tint vignette green/blue channels (subtracting into red): {color:?}"
        );

        game.ui.hide_gui = true;
        let quads_hidden = game.screen_quads(1.0, [1280, 800], 1.0, hand_proj);
        assert!(
            quads_hidden.iter().all(|q| q.blend != Blend::Vignette),
            "F1 hide_gui must suppress Hud vignette overlay"
        );
    });
}

/// Item 6: streaming center, particles, F3 debug entries/gizmos, and spectator crosshair (26.3).
#[test]
#[ignore = "daily"]
fn camera_consumers_streaming_particles_debug_and_spectator_crosshair_26_3() {
    let witness = parity_witness();
    let consumers = &witness["camera_consumers"];
    let stream = &consumers["streaming_and_particles"];
    let debug = &consumers["debug"];
    let cross = &consumers["spectator_crosshair"];
    let want_chunk = (
        stream["camera_chunk"][0].as_i64().unwrap() as i32,
        stream["camera_chunk"][1].as_i64().unwrap() as i32,
    );
    let want_block_y = stream["entity_block_y"].as_i64().unwrap() as i32;
    let want_cam_pos_bits = vec3_bits_expected(&stream["camera_pos_bits"]);
    let want_xyz_line = debug["position_lines"][0].as_str().unwrap();
    let want_biome_line = debug["biome_line"].as_str().unwrap();
    let want_light_line = debug["light_line"].as_str().unwrap();
    let want_min_x = hex_f64(&debug["chunk_borders"]["min_x_bits"]);
    let want_max_x = hex_f64(&debug["chunk_borders"]["max_x_bits"]);
    let want_min_z = hex_f64(&debug["chunk_borders"]["min_z_bits"]);
    let want_max_z = hex_f64(&debug["chunk_borders"]["max_z_bits"]);
    let want_player_box = debug["entity_hitboxes"]["player_box_emitted_in_first_person"]
        .as_bool()
        .unwrap();
    let want_cam_box = debug["entity_hitboxes"]["camera_interaction_box_emitted_in_first_person"]
        .as_bool()
        .unwrap();
    let want_cart_crosshair = cross["cart_only_crosshair"].as_bool().unwrap();
    let want_occluded_crosshair = cross["with_interaction_crosshair"].as_bool().unwrap();

    with_client_game(|game| {
        let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
        let plains = *world_biomes!(game).index.get("minecraft:plains").unwrap();
        let desert = *world_biomes!(game).index.get("minecraft:desert").unwrap();
        game.world
            .chunks
            .insert((0, 0), Arc::new(make_chunk(0, 0, plains, 0xF0)));
        game.world
            .chunks
            .insert((6, 6), Arc::new(make_chunk(6, 6, desert, 0x00)));

        add_entity(
            game,
            31,
            "minecraft:interaction",
            [96.5, 300.0, 96.5],
            0.0,
            0.0,
        );
        sync_interaction_size(game, 31, 0.1, 1.9059);
        game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 31 }))
            .unwrap();
        game.view = Default::default();
        for _ in 0..40 {
            game.tick_fov();
        }
        game.camera_view(1.0);

        // 1. Streaming center follows camera entity.
        game.update_world();
        assert_eq!(
            game.streaming.center, want_chunk,
            "streaming.center must follow camera"
        );
        assert_eq!(
            game.streaming.center_y, want_block_y,
            "streaming.center_y must follow camera"
        );

        // 2. Particle camera follows camera entity eye (exact 26.3 camera.position() bits).
        game.tick_particles();
        assert_eq!(
            game.particles.camera.map(f64::to_bits),
            want_cam_pos_bits,
            "particles.camera must match 26.3 camera.position() bits"
        );

        // 3. F3 debug entries report camera entity position, biome, and light.
        game.debug = true;
        game.debug_entries
            .set_status("player_position", Status::InOverlay);
        game.debug_entries.set_status("biome", Status::InOverlay);
        game.debug_entries
            .set_status("light_levels", Status::InOverlay);
        let mut displayer = Displayer::default();
        game.debug_entry_lines(&mut displayer, [1280, 800]);
        let (left, right) = displayer.columns(&[]);
        let all_lines: Vec<String> = left.into_iter().chain(right).collect();
        let joined = all_lines.join("\n");
        assert!(
            joined.contains(want_xyz_line),
            "F3 position must match 26.3 witness '{want_xyz_line}', got:\n{joined}"
        );
        assert!(
            joined.contains(want_biome_line),
            "F3 biome must match 26.3 witness '{want_biome_line}', got:\n{joined}"
        );
        assert!(
            joined.contains(want_light_line),
            "F3 light must match 26.3 witness '{want_light_line}', got:\n{joined}"
        );

        // 4. F3 gizmos: chunk_borders bounds match 26.3 ChunkBorderRenderer; hitboxes follow EntityHitboxDebugRenderer.
        game.debug_entries
            .set_status("chunk_borders", Status::AlwaysOn);
        game.debug_entries
            .set_status("entity_hitboxes", Status::Never);
        let border_gizmos = game.debug_gizmos(1.0, [96.5, 301.62, 96.5]);
        let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut min_z, mut max_z) = (f64::INFINITY, f64::NEG_INFINITY);
        for l in &border_gizmos.lines {
            min_x = min_x.min(l.start[0]).min(l.end[0]);
            max_x = max_x.max(l.start[0]).max(l.end[0]);
            min_z = min_z.min(l.start[2]).min(l.end[2]);
            max_z = max_z.max(l.start[2]).max(l.end[2]);
        }
        assert_eq!(min_x.to_bits(), want_min_x, "chunk_borders min_x");
        assert_eq!(max_x.to_bits(), want_max_x, "chunk_borders max_x");
        assert_eq!(min_z.to_bits(), want_min_z, "chunk_borders min_z");
        assert_eq!(max_z.to_bits(), want_max_z, "chunk_borders max_z");

        game.debug_entries
            .set_status("chunk_borders", Status::Never);
        game.debug_entries
            .set_status("entity_hitboxes", Status::AlwaysOn);
        let hitbox_gizmos = game.debug_gizmos(1.0, [96.5, 301.62, 96.5]);
        let has_player_box = hitbox_gizmos.lines.iter().any(|l| {
            l.start[0] >= 0.0 && l.start[0] <= 1.0 && l.start[1] >= 64.0 && l.start[1] <= 66.0
        });
        assert_eq!(
            has_player_box, want_player_box,
            "local player hitbox in FirstPerson while spectating another entity"
        );
        let has_camera_interaction_box = hitbox_gizmos.lines.iter().any(|l| {
            l.start[0] >= 96.4 && l.start[0] <= 96.6 && l.start[1] >= 300.0 && l.start[1] <= 302.0
        });
        assert_eq!(
            has_camera_interaction_box, want_cam_box,
            "camera entity hitbox in FirstPerson"
        );

        // 5. Spectator crosshair: a nearer Interaction blocks a chest minecart behind it.
        game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 1 }))
            .unwrap();
        game.yaw = 0.0;
        game.pitch = 0.0;
        add_entity(
            game,
            40,
            "minecraft:chest_minecart",
            [0.5, 65.2, 3.5],
            0.0,
            0.0,
        );
        game.refresh_client_pick(1.0);
        assert_eq!(
            game.spectator_crosshair(),
            want_cart_crosshair,
            "unobstructed chest minecart spectator crosshair"
        );
        add_entity(
            game,
            41,
            "minecraft:interaction",
            [0.5, 65.0, 2.0],
            0.0,
            0.0,
        );
        game.refresh_client_pick(1.0);
        assert_eq!(
            game.spectator_crosshair(),
            want_occluded_crosshair,
            "nearer Interaction is not a MenuProvider and must occlude chest minecart"
        );
    });
}

/// Item 6: frozen-level camera partial tick matches 26.3 `Camera.getCameraEntityPartialTicks`.
#[test]
#[ignore = "daily"]
fn frozen_camera_partial_tick_matches_26_3_witness() {
    let witness = parity_witness();
    let frozen = &witness["camera_consumers"]["frozen_partial"];
    with_client_game(|game| {
        let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
        add_entity(
            game,
            2,
            "minecraft:interaction",
            [0.5, 300.0, 0.5],
            0.0,
            0.0,
        );
        add_entity(game, 3, "minecraft:creeper", [5.0, 64.0, 5.0], 0.0, 0.0);
        add_entity(game, 4, "minecraft:minecart", [5.0, 64.0, 5.0], 0.0, 0.0);
        game.handle_packet(C::SetPassengers(play::SetPassengers {
            vehicle: 4,
            passengers: vec![1],
        }))
        .unwrap();

        game.clock.reset();
        game.clock.set_frozen(true);
        game.clock.tick_world_runs();
        game.clock.end_frame(false);
        // 3ms / 8ms = 0.375 exactly (0x3ec00000), matching real_partial_bits in witness.
        let ran = game.clock.advance(std::time::Duration::from_millis(3), 8.0);
        assert_eq!(ran, 0);
        assert_eq!(
            (game.clock.camera_fraction() as f32).to_bits(),
            hex_f32(&frozen["real_partial_bits"]),
            "setup residual must match real_partial_bits"
        );

        for (label, cam_id, key) in [
            ("local_player", 1, "local_player_partial_bits"),
            ("interaction", 2, "interaction_partial_bits"),
            ("creeper", 3, "creeper_partial_bits"),
            (
                "minecart_with_player",
                4,
                "minecart_with_player_partial_bits",
            ),
        ] {
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: cam_id }))
                .unwrap();
            assert_eq!(
                (game.camera_partial_tick() as f32).to_bits(),
                hex_f32(&frozen[key]),
                "frozen camera partial tick for {label}"
            );
        }
    });
}

/// Item 6: block outline requires a player camera entity (`GameRenderer.shouldRenderBlockOutline`).
#[test]
#[ignore = "daily"]
fn block_outline_requires_player_camera_26_3() {
    let witness = parity_witness();
    let outline = &witness["camera_consumers"]["block_outline"];
    with_client_game(|game| {
        let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
        game.handle_packet(C::GameEvent(play::GameEvent {
            event: 3,
            param: 1.0,
        }))
        .unwrap();
        game.ui.hide_gui = false;
        add_entity(
            game,
            2,
            "minecraft:interaction",
            [0.5, 300.0, 0.5],
            0.0,
            0.0,
        );
        add_entity(game, 3, "minecraft:creeper", [5.0, 64.0, 5.0], 0.0, 0.0);

        for (label, cam_id) in [("local_player", 1), ("interaction", 2), ("creeper", 3)] {
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: cam_id }))
                .unwrap();
            assert_eq!(
                game.should_render_block_outline(),
                outline[label].as_bool().unwrap(),
                "should_render_block_outline for {label}"
            );
        }
    });
}

/// Item 14: culling near plane uses max(fov, option_fov) while scoping with a spyglass (26.3).
#[test]
#[ignore = "daily"]
fn culling_near_plane_uses_max_of_fov_and_option_fov_26_3() {
    let witness = parity_witness();
    let case = witness["cull_frustum"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "zoomed_fov_30_opt_70")
        .unwrap();
    let near_max_x = f64::from_bits(hex_f64(&case["near_plane_bounds_bits"][3]));
    assert!(
        0.99 + near_max_x > 1.0,
        "26.3 70-deg cull near plane must cross x=1.0 from x=0.99"
    );

    with_client_game(|game| {
        // Bottom stone_slab at [1,65,0] uses NEAR_PLANE_INTERSECTS_OUTLINE: 70-deg hits, 7-deg misses.
        let _server = setup_remote_overworld(game, 1, [0.99, 63.6, 0.5], true);
        game.world
            .chunks
            .insert((0, 0), Arc::new(make_chunk(0, 0, 0, 0xF0)));
        let slab = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone_slab")
            .unwrap();
        game.world.set(1, 65, 0, slab).unwrap();

        game.yaw = 0.0;
        game.pitch = 0.0;
        game.settings.fov = 70.0;
        game.ui.hide_gui = true;
        let hand_proj = sunrise_client::camera::hand_projection(70.0, 1280.0, 800.0, 512.0);
        for _ in 0..20 {
            game.tick_fov();
        }
        let baseline_quads = game.screen_quads(1.0, [1280, 800], 1.0, hand_proj);
        assert!(
            baseline_quads
                .iter()
                .any(|q| q.blend == Blend::Alpha && !q.world_tint),
            "baseline 70-deg near plane must intersect stone_slab at x=1.0"
        );

        game.inventory[0] = Some(ItemStack::new("minecraft:spyglass", 1));
        game.selected = 0;
        game.using = Some((0, 5));
        for _ in 0..20 {
            game.tick_fov();
        }
        let eye = [0.99, 65.22, 0.5];
        assert!(
            game.fov_degrees(1.0, eye) < 15.0,
            "spyglass must narrow render FOV well below option FOV 70"
        );

        let quads = game.screen_quads(1.0, [1280, 800], 1.0, hand_proj);
        assert!(
            quads
                .iter()
                .any(|q| q.blend == Blend::Alpha && !q.world_tint),
            "cull near plane must use max(fov, option_fov)=70 so stone_slab at x=1.0 is view-blocking"
        );
    });
}

fn add_entity_with_data(
    game: &mut Game<'_>,
    id: i32,
    kind: &str,
    position: [f64; 3],
    yaw: f32,
    pitch: f32,
    data: i32,
) {
    let uuid = id as u128 + 1000;
    if kind == "minecraft:player" {
        let r = game.remote.as_mut().unwrap();
        r.players.insert(uuid, format!("Remote{id}"));
        r.level_players.insert(id, uuid);
    }
    let kind_id = entity_type_id(game, kind);
    let y = pack_degrees(yaw);
    let p = pack_degrees(pitch);
    game.handle_packet(C::AddEntity(play::AddEntity {
        id,
        uuid,
        kind: kind_id,
        position,
        velocity: [0.0; 3],
        pitch: p,
        yaw: y,
        head_yaw: y,
        data,
    }))
    .unwrap();
}

/// Parity #1: any spectated sleeping living entity (RemotePlayer or mob, on a real bed or a
/// non-bed block) adopts the vanilla 26.3 bed view (`bed_yaw - 180` or `0`, pitch `0`, `+0.15`)
/// and stays visible in first person (`LevelExtractor` does not hide a sleeping camera entity).
#[test]
#[ignore = "daily"]
fn sleeping_spectated_living_entity_bed_view_and_visibility_26_3() {
    let witness = parity_witness();
    let cases = witness["sleeping_spectated_camera"]["cases"]
        .as_array()
        .unwrap();
    assert_eq!(cases.len(), 9);

    with_client_game(|game| {
        let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
        game.world
            .chunks
            .insert((0, 0), Arc::new(make_chunk(0, 0, 0, 0xF0)));
        let registry = game.pack.assets.registry;
        let bed_block = |facing: &str| {
            registry
                .resolve(&sunrise_world::chunk::BlockState {
                    name: "minecraft:red_bed".into(),
                    properties: [("facing", facing), ("occupied", "false"), ("part", "head")]
                        .into_iter()
                        .map(|(k, v)| (k.into(), v.into()))
                        .collect(),
                })
                .unwrap()
        };
        let furnace_west = registry
            .resolve(&sunrise_world::chunk::BlockState {
                name: "minecraft:furnace".into(),
                properties: [("facing", "west"), ("lit", "false")]
                    .into_iter()
                    .map(|(k, v)| (k.into(), v.into()))
                    .collect(),
            })
            .unwrap();
        let stone = registry.default_state("minecraft:stone").unwrap();

        let mut next_id = 500;
        for tc in cases {
            let name = tc["name"].as_str().unwrap();
            let kind = tc["kind"].as_str().unwrap();
            let sleeping = tc["sleeping"].as_bool().unwrap();
            let state = match name {
                "remote_player_bed_south" | "remote_player_awake" => bed_block("south"),
                "remote_player_bed_west" | "zombie_bed_west" => bed_block("west"),
                "remote_player_bed_north" => bed_block("north"),
                "remote_player_bed_east" => bed_block("east"),
                "remote_player_furnace_non_bed" => furnace_west,
                "remote_player_stone_non_bed" | "zombie_stone_non_bed" => stone,
                other => panic!("unexpected sleeping case {other}"),
            };
            game.world.set(10, 64, 10, state).unwrap();

            let id = next_id;
            next_id += 1;
            add_entity_with_data(game, id, kind, [10.5, 64.6875, 10.5], 37.5, -15.0, 0);
            game.handle_packet(C::SetEntityData(play::SetEntityData {
                id,
                values: vec![
                    (
                        6,
                        serializer::POSE,
                        EntityDataValue::Id(if sleeping { 2 } else { 0 }),
                    ),
                    (
                        14,
                        serializer::OPTIONAL_BLOCK_POS,
                        EntityDataValue::OptPos(sleeping.then_some([10, 64, 10])),
                    ),
                ],
            }))
            .unwrap();
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: id }))
                .unwrap();

            game.view = Default::default();
            game.tick_fov();
            let (pos1, _, _) = game.camera_view(1.0).expect("1-tick camera view");
            assert_eq!(
                pos1.to_array().map(f64::to_bits),
                vec3_bits_expected(&tc["camera_pos_after_1_tick_bits"]),
                "{name}: 1-tick camera_pos_bits"
            );

            for _ in 0..40 {
                game.tick_fov();
            }
            let (pos, yaw, pitch) = game.camera_view(1.0).expect("settled camera view");
            assert_eq!(
                pos.to_array().map(f64::to_bits),
                vec3_bits_expected(&tc["camera_pos_settled_bits"]),
                "{name}: settled camera_pos_bits"
            );
            assert_eq!(
                yaw.to_bits(),
                hex_f32(&tc["camera_yRot_bits"]),
                "{name}: camera_yRot_bits"
            );
            assert_eq!(
                pitch.to_bits(),
                hex_f32(&tc["camera_xRot_bits"]),
                "{name}: camera_xRot_bits"
            );
            let q = game.view.rotation;
            assert_eq!(
                [q.x.to_bits(), q.y.to_bits(), q.z.to_bits(), q.w.to_bits()],
                [
                    hex_f32(&tc["camera_rotation_bits"][0]),
                    hex_f32(&tc["camera_rotation_bits"][1]),
                    hex_f32(&tc["camera_rotation_bits"][2]),
                    hex_f32(&tc["camera_rotation_bits"][3]),
                ],
                "{name}: camera_rotation_bits"
            );
            let subject = game.camera_subject().expect("camera subject");
            assert_eq!(
                subject.is_sleeping,
                tc["state_is_sleeping"].as_bool().unwrap(),
                "{name}: subject.is_sleeping"
            );
            assert_eq!(
                game.hides_as_camera(id as u32),
                tc["hidden_in_first_person"].as_bool().unwrap(),
                "{name}: hides_as_camera in first person"
            );
        }
    });
}

/// Parity #3: `Camera.tickFov` reads the camera player's own `getFieldOfViewModifier` (movement
/// speed, bow draw, offhand spyglass, `fovEffectScale`, first- vs third-person) and isolates the
/// camera from the local player's death/speed/scoping while still applying the camera entity's own
/// death squeeze.
#[test]
#[ignore = "daily"]
fn camera_player_fov_modifier_and_death_isolation_26_3() {
    let witness = parity_witness();
    let cases = witness["camera_player_fov_modifier"]["cases"]
        .as_array()
        .unwrap();
    let find_case = |name: &str| {
        cases
            .iter()
            .find(|c| c["name"] == name)
            .unwrap_or_else(|| panic!("missing fov case {name}"))
    };
    let item_id = |game: &Game<'_>, name: &str| {
        game.remote
            .as_ref()
            .unwrap()
            .builtin
            .get("minecraft:item")
            .unwrap()
            .id(name)
            .unwrap()
    };
    let speed_attr_id = |game: &Game<'_>| {
        game.motion
            .attributes
            .id("minecraft:movement_speed")
            .unwrap()
            .registry_id()
    };

    // Case 1: sprinting speed (base 0.1 + 0.3 ADD_MULTIPLIED_TOTAL = 0.13), fovEffectScale = 1.0
    {
        let tc = find_case("sprinting_speed_scale_1_0");
        with_client_game(|game| {
            let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
            game.settings.file.set("fovEffectScale", OptV::Double(1.0));
            add_entity_with_data(game, 20, "minecraft:player", [5.5, 64.0, 5.5], 0.0, 0.0, 0);
            let attr = speed_attr_id(game);
            game.handle_packet(C::UpdateAttributes(play::UpdateAttributes {
                id: 20,
                values: vec![play::AttributeSnapshot {
                    attribute: attr,
                    base: 0.1,
                    modifiers: vec![play::AttributeModifier {
                        id: "minecraft:sprinting".into(),
                        amount: 0.3,
                        operation: 2,
                    }],
                }],
            }))
            .unwrap();
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 20 }))
                .unwrap();
            game.view = Default::default();
            game.tick_fov();
            let cam_pos = [5.5, 65.62, 5.5];
            assert_eq!(
                game.fov_degrees(1.0, cam_pos).to_bits(),
                hex_f32(&tc["fov_after_1_tick_p1_bits"]),
                "sprinting_speed_scale_1_0: 1-tick p=1.0 fov"
            );
            assert_eq!(
                game.fov_degrees(0.5, cam_pos).to_bits(),
                hex_f32(&tc["fov_after_1_tick_p05_bits"]),
                "sprinting_speed_scale_1_0: 1-tick p=0.5 fov"
            );
            assert_eq!(
                game.hud_fov_degrees(1.0, cam_pos).to_bits(),
                hex_f32(&tc["hud_after_1_tick_p1_bits"]),
                "sprinting_speed_scale_1_0: 1-tick hud fov"
            );
            for _ in 0..39 {
                game.tick_fov();
            }
            assert_eq!(
                game.fov_degrees(1.0, cam_pos).to_bits(),
                hex_f32(&tc["fov_settled_bits"]),
                "sprinting_speed_scale_1_0: settled fov"
            );
            assert_eq!(
                game.hud_fov_degrees(1.0, cam_pos).to_bits(),
                hex_f32(&tc["hud_settled_bits"]),
                "sprinting_speed_scale_1_0: settled hud fov"
            );
        });
    }

    // Case 2: speed = 0.2, fovEffectScale = 0.5
    {
        let tc = find_case("speed_0_2_scale_0_5");
        with_client_game(|game| {
            let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
            game.settings.file.set("fovEffectScale", OptV::Double(0.5));
            add_entity_with_data(game, 21, "minecraft:player", [5.5, 64.0, 5.5], 0.0, 0.0, 0);
            let attr = speed_attr_id(game);
            game.handle_packet(C::UpdateAttributes(play::UpdateAttributes {
                id: 21,
                values: vec![play::AttributeSnapshot {
                    attribute: attr,
                    base: 0.2,
                    modifiers: vec![],
                }],
            }))
            .unwrap();
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 21 }))
                .unwrap();
            game.view = Default::default();
            for _ in 0..40 {
                game.tick_fov();
            }
            let cam_pos = [5.5, 65.62, 5.5];
            assert_eq!(
                game.fov_degrees(1.0, cam_pos).to_bits(),
                hex_f32(&tc["fov_settled_bits"]),
                "speed_0_2_scale_0_5: settled fov"
            );
            assert_eq!(
                game.hud_fov_degrees(1.0, cam_pos).to_bits(),
                hex_f32(&tc["hud_settled_bits"]),
                "speed_0_2_scale_0_5: settled hud fov"
            );
        });
    }

    // Case 3: bow draw in main hand at 0, 5, 10, 20, 25 ticks
    {
        let tc = find_case("bow_draw_mainhand");
        with_client_game(|game| {
            let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
            game.settings.file.set("fovEffectScale", OptV::Double(1.0));
            add_entity_with_data(game, 22, "minecraft:player", [5.5, 64.0, 5.5], 0.0, 0.0, 0);
            let bow = WireStack::new(item_id(game, "minecraft:bow"), 1);
            game.handle_packet(C::SetEquipment(play::SetEquipment {
                id: 22,
                slots: vec![(0, Some(bow))],
            }))
            .unwrap();
            game.handle_packet(C::SetEntityData(play::SetEntityData {
                id: 22,
                values: vec![(8, serializer::BYTE, EntityDataValue::Byte(1))],
            }))
            .unwrap();
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 22 }))
                .unwrap();
            let mut elapsed = 0i64;
            for sample in tc["samples"].as_array().unwrap() {
                let target_tick = sample["elapsed_ticks"].as_i64().unwrap();
                while elapsed < target_tick {
                    game.tick_remote_entities(true);
                    elapsed += 1;
                }
                game.view = Default::default();
                for _ in 0..40 {
                    game.tick_fov();
                }
                assert_eq!(
                    game.fov_degrees(1.0, [5.5, 65.62, 5.5]).to_bits(),
                    hex_f32(&sample["fov_settled_bits"]),
                    "bow_draw_mainhand at elapsed_ticks={target_tick}"
                );
            }
        });
    }

    // Cases 4 & 5: spyglass in offhand (first person vs third person), fovEffectScale = 0.5
    for (name, ct) in [
        ("spyglass_offhand_first_person", CameraType::FirstPerson),
        ("spyglass_offhand_third_person", CameraType::ThirdPersonBack),
    ] {
        let tc = find_case(name);
        with_client_game(|game| {
            let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
            game.settings.file.set("fovEffectScale", OptV::Double(0.5));
            game.camera_type = ct;
            add_entity_with_data(game, 23, "minecraft:player", [5.5, 64.0, 5.5], 0.0, 0.0, 0);
            let spyglass = WireStack::new(item_id(game, "minecraft:spyglass"), 1);
            game.handle_packet(C::SetEquipment(play::SetEquipment {
                id: 23,
                slots: vec![(1, Some(spyglass))],
            }))
            .unwrap();
            game.handle_packet(C::SetEntityData(play::SetEntityData {
                id: 23,
                values: vec![(8, serializer::BYTE, EntityDataValue::Byte(3))],
            }))
            .unwrap();
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 23 }))
                .unwrap();
            game.view = Default::default();
            for _ in 0..40 {
                game.tick_fov();
            }
            let cam_pos = [5.5, 65.62, 5.5];
            assert_eq!(
                game.fov_degrees(1.0, cam_pos).to_bits(),
                hex_f32(&tc["fov_settled_bits"]),
                "{name}: settled fov"
            );
            assert_eq!(
                game.hud_fov_degrees(1.0, cam_pos).to_bits(),
                hex_f32(&tc["hud_settled_bits"]),
                "{name}: settled hud fov"
            );
        });
    }

    // Cases 6 & 7: local player dying + fast + scoping must not affect healthy spectated camera
    for (name, kind, id) in [
        (
            "local_dying_spectating_healthy_remote_player",
            "minecraft:player",
            24,
        ),
        (
            "local_dying_spectating_healthy_creeper",
            "minecraft:creeper",
            25,
        ),
    ] {
        let tc = find_case(name);
        with_client_game(|game| {
            let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
            game.settings.file.set("fovEffectScale", OptV::Double(1.0));
            let local = game.player.as_mut().unwrap();
            local.set_health(0.0, false).unwrap();
            local.speed = 0.3;
            game.ui.death_time = 10;
            game.inventory[0] = Some(ItemStack::new("minecraft:spyglass", 1));
            game.selected = 0;
            game.using = Some((0, 5));

            add_entity_with_data(game, id, kind, [5.5, 64.0, 5.5], 0.0, 0.0, 0);
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: id }))
                .unwrap();
            game.view = Default::default();
            for _ in 0..40 {
                game.tick_fov();
            }
            let cam_pos = [5.5, 65.62, 5.5];
            assert_eq!(
                game.fov_degrees(0.5, cam_pos).to_bits(),
                hex_f32(&tc["fov_settled_bits"]),
                "{name}: settled fov"
            );
            assert_eq!(
                game.hud_fov_degrees(0.5, cam_pos).to_bits(),
                hex_f32(&tc["hud_settled_bits"]),
                "{name}: settled hud fov"
            );
        });
    }

    // Case 8: local player healthy, spectating dying + sprinting RemotePlayer
    {
        let tc = find_case("local_healthy_spectating_dying_sprinting_remote_player");
        with_client_game(|game| {
            let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
            game.settings.file.set("fovEffectScale", OptV::Double(1.0));
            add_entity_with_data(game, 26, "minecraft:player", [5.5, 64.0, 5.5], 0.0, 0.0, 0);
            let attr = speed_attr_id(game);
            game.handle_packet(C::UpdateAttributes(play::UpdateAttributes {
                id: 26,
                values: vec![play::AttributeSnapshot {
                    attribute: attr,
                    base: 0.1,
                    modifiers: vec![play::AttributeModifier {
                        id: "minecraft:sprinting".into(),
                        amount: 0.3,
                        operation: 2,
                    }],
                }],
            }))
            .unwrap();
            game.handle_packet(C::SetEntityData(play::SetEntityData {
                id: 26,
                values: vec![(9, serializer::FLOAT, EntityDataValue::Float(0.0))],
            }))
            .unwrap();
            game.mobs
                .iter_mut()
                .find(|m| m.id == 26)
                .unwrap()
                .death_time = 10;
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: 26 }))
                .unwrap();
            game.view = Default::default();
            for _ in 0..40 {
                game.tick_fov();
            }
            let cam_pos = [5.5, 65.62, 5.5];
            assert_eq!(
                game.fov_degrees(0.5, cam_pos).to_bits(),
                hex_f32(&tc["fov_p05_bits"]),
                "dying sprinting remote player p=0.5 fov"
            );
            assert_eq!(
                game.hud_fov_degrees(0.5, cam_pos).to_bits(),
                hex_f32(&tc["hud_p05_bits"]),
                "dying sprinting remote player p=0.5 hud fov"
            );
            assert_eq!(
                game.fov_degrees(1.0, cam_pos).to_bits(),
                hex_f32(&tc["fov_p1_bits"]),
                "dying sprinting remote player p=1.0 fov"
            );
            assert_eq!(
                game.hud_fov_degrees(1.0, cam_pos).to_bits(),
                hex_f32(&tc["hud_p1_bits"]),
                "dying sprinting remote player p=1.0 hud fov"
            );
        });
    }
}

/// Parity #33: `Minecraft.pick` / `ProjectileUtil.getEntityHitResult` excludes the camera entity
/// itself for every pickable entity family (boats, item frames, minecarts, TNT, interactions,
/// end crystals, paintings, armor stands, mobs) and shoots from the camera entity's exact eye and
/// view direction so a target ahead is picked instead of self.
#[test]
#[ignore = "daily"]
fn camera_entity_excluded_from_pick_for_all_pickable_kinds_26_3() {
    let witness = parity_witness();
    let cases = witness["camera_pick_exclusion"]["cases"]
        .as_array()
        .unwrap();
    assert_eq!(cases.len(), 11);

    for (idx, tc) in cases.iter().enumerate() {
        let name = tc["name"].as_str().unwrap();
        let kind = tc["kind"].as_str().unwrap();
        let data = tc["data"].as_i64().unwrap() as i32;
        let spawn_pos = vec3_from_bits(&tc["spawn_pos_bits"]);
        let target_feet = vec3_from_bits(&tc["target_feet_bits"]);
        let cam_id = 700 + idx as i32;

        with_client_game(|game| {
            let _server = setup_remote_overworld(game, 1, [50.5, 64.0, 50.5], true);
            game.handle_packet(C::GameEvent(play::GameEvent {
                event: 3,
                param: 3.0,
            }))
            .unwrap();
            game.entity_reach = 4.5;

            add_entity_with_data(game, cam_id, kind, spawn_pos, 0.0, 0.0, data);
            if kind == "minecraft:interaction" {
                sync_interaction_size(game, cam_id, 1.0, 1.0);
            }
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: cam_id }))
                .unwrap();

            let snap = game
                .spectated_camera_snapshot()
                .unwrap_or_else(|| panic!("{name}: camera snapshot missing"));
            assert_eq!(
                snap.position.map(f64::to_bits),
                vec3_bits_expected(&tc["camera_pos_bits"]),
                "{name}: camera_pos_bits"
            );
            assert_eq!(
                snap.eye_height.to_bits(),
                hex_f32(&tc["camera_eye_height_bits"]),
                "{name}: camera_eye_height_bits"
            );
            assert_eq!(
                snap.y_rot.to_bits(),
                hex_f32(&tc["camera_yRot_bits"]),
                "{name}: camera_yRot_bits"
            );
            assert_eq!(
                snap.x_rot.to_bits(),
                hex_f32(&tc["camera_xRot_bits"]),
                "{name}: camera_xRot_bits"
            );

            assert!(
                game.native_lifecycle.epoch().is_some(),
                "{name}: world epoch must be live for client pick"
            );
            game.refresh_client_pick(1.0);
            assert_eq!(
                game.crosshair_entity_id(),
                None,
                "{name}: camera entity must be excluded from crosshair_entity_id when alone"
            );
            assert_eq!(
                game.spectator_click_target(),
                None,
                "{name}: camera entity must be excluded from spectator_click_target when alone"
            );

            add_entity_with_data(game, 900, "minecraft:creeper", target_feet, 0.0, 0.0, 0);
            game.refresh_client_pick(1.0);
            let want = tc["hit_with_target_id"].as_i64().unwrap() as i32;
            assert_eq!(
                game.crosshair_entity_id(),
                Some(want),
                "{name}: crosshair_entity_id must hit target past excluded camera entity"
            );
            assert_eq!(
                game.spectator_click_target(),
                Some(want),
                "{name}: spectator_click_target must hit target past excluded camera entity"
            );
        });
    }
}

/// Addendum (a): `LevelExtractor.extractVisibleEntities` never draws `LocalPlayer` unless it is
/// the camera entity (and in third person or sleeping), and hides the camera entity in first
/// person whatever its kind (body, flames, shadows, name tag).
#[test]
#[ignore = "daily"]
fn camera_entity_hidden_in_first_person_for_all_kinds_and_local_player_hidden_26_3() {
    use sunrise_client::camera::CameraType;
    use sunrise_core::nbt::Tag;

    let witness = parity_witness();
    let cases = witness["camera_visibility_all_kinds"]["cases"]
        .as_array()
        .unwrap();
    let camera_types = [
        ("third_person_back", CameraType::ThirdPersonBack),
        ("third_person_front", CameraType::ThirdPersonFront),
        ("first_person", CameraType::FirstPerson),
    ];
    let cam = glam::DVec3::new(5.5, 64.5, 8.5);
    let mut failures = Vec::new();
    let mut next_id = 800i32;

    for tc in cases {
        let name = tc["name"].as_str().unwrap();
        let kind = tc["kind"].as_str().unwrap();
        let data = tc["data"].as_i64().unwrap() as i32;
        let local_sleeping = tc["local_sleeping"].as_bool().unwrap();
        let camera_on_self = tc["camera_on_self"].as_bool().unwrap();

        with_client_game(|game| {
            let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], false);
            game.view_frustum = None;
            game.settings.entity_shadows = false;
            game.settings.extras.own_name_tag = false;
            game.tick_player_model();
            game.sleeping = local_sleeping.then_some([0, 64, 0]);

            let cam_id = if camera_on_self {
                1
            } else {
                let id = next_id;
                next_id += 1;
                let stone_state = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:stone")
                    .unwrap()
                    .0 as i32;
                let stone_item = game
                    .remote
                    .as_ref()
                    .unwrap()
                    .builtin
                    .get("minecraft:item")
                    .unwrap()
                    .id("minecraft:stone")
                    .unwrap();
                let spawn_data = if kind == "minecraft:falling_block" {
                    stone_state
                } else {
                    data
                };
                add_entity_with_data(game, id, kind, [5.5, 64.0, 5.5], 0.0, 0.0, spawn_data);
                match kind {
                    "minecraft:item" => {
                        game.handle_packet(C::SetEntityData(play::SetEntityData {
                            id,
                            values: vec![(
                                8,
                                serializer::ITEM_STACK,
                                EntityDataValue::Item(Some(WireStack::new(stone_item, 1))),
                            )],
                        }))
                        .unwrap();
                    }
                    "minecraft:block_display" => {
                        game.handle_packet(C::SetEntityData(play::SetEntityData {
                            id,
                            values: vec![(
                                23,
                                serializer::BLOCK_STATE,
                                EntityDataValue::Id(stone_state),
                            )],
                        }))
                        .unwrap();
                        game.tick_text_displays();
                    }
                    "minecraft:item_display" => {
                        game.handle_packet(C::SetEntityData(play::SetEntityData {
                            id,
                            values: vec![(
                                23,
                                serializer::ITEM_STACK,
                                EntityDataValue::Item(Some(WireStack::new(stone_item, 1))),
                            )],
                        }))
                        .unwrap();
                        game.tick_text_displays();
                    }
                    "minecraft:text_display" => {
                        game.handle_packet(C::SetEntityData(play::SetEntityData {
                            id,
                            values: vec![(
                                23,
                                serializer::COMPONENT,
                                EntityDataValue::Text(Tag::String("HI".into())),
                            )],
                        }))
                        .unwrap();
                        game.tick_text_displays();
                    }
                    _ => {}
                }
                game.handle_packet(C::SetCamera(play::SetCamera { camera_id: id }))
                    .unwrap();
                id
            };

            for (ct_name, ct) in camera_types {
                let want_local = tc["views"][ct_name]["local_player_extracted"]
                    .as_bool()
                    .unwrap();
                let want_cam = tc["views"][ct_name]["camera_entity_extracted"]
                    .as_bool()
                    .unwrap();
                game.camera_type = ct;
                if game.player_model_shown() != want_local {
                    failures.push(format!(
                        "{name} {ct_name}: player_model_shown={} want {want_local}",
                        game.player_model_shown()
                    ));
                }
                let mut additive = Vec::new();
                let (verts, _, _, _) =
                    game.entity_geometry(cam, 1.0, None, Vec::new(), &mut additive);
                let has_local_run = game
                    .entity_runs
                    .iter()
                    .any(|r| r.family == crate::game::entity::DrawFamily::LocalPlayer);
                if has_local_run != want_local {
                    failures.push(format!(
                        "{name} {ct_name}: LocalPlayer run present={has_local_run} want {want_local}"
                    ));
                }
                if !camera_on_self {
                    let drew = !verts.is_empty()
                        || !additive.is_empty()
                        || !game.see_through_vertices.is_empty();
                    if drew != want_cam {
                        failures.push(format!(
                            "{name} ({kind}, id={cam_id}) {ct_name}: drew={drew} (verts={}, additive={}, see_through={}) want {want_cam}",
                            verts.len(),
                            additive.len(),
                            game.see_through_vertices.len(),
                        ));
                    }
                }
            }

            // Local player own_name_tag must never show while camera is on another entity.
            if !camera_on_self && name == "creeper" {
                game.camera_type = CameraType::ThirdPersonBack;
                game.settings.extras.own_name_tag = true;
                let own_name = game.own_profile_name();
                let tags = game.shown_name_tags(glam::DVec3::new(0.5, 65.5, 4.5));
                if tags.iter().any(|t| t == &own_name) {
                    failures.push(format!(
                        "{name}: local player own_name_tag shown while spectating another entity: {tags:?}"
                    ));
                }
            }
        });
    }

    // Flames, shadows, and armor-stand name tags must also be suppressed when camera is hidden in first person.
    with_client_game(|game| {
        let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], false);
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        game.world
            .chunks
            .insert((0, 0), Arc::new(make_chunk(0, 0, 0, 0xF0)));
        game.world.set(5, 63, 5, stone).unwrap();
        game.view_frustum = None;
        game.settings.entity_shadows = true;
        game.settings.extras.own_name_tag = false;
        game.tick_player_model();

        let stone_item = game
            .remote
            .as_ref()
            .unwrap()
            .builtin
            .get("minecraft:item")
            .unwrap()
            .id("minecraft:stone")
            .unwrap();

        for (label, kind, id) in [
            ("burning_shadow_creeper", "minecraft:creeper", 920),
            ("burning_shadow_item", "minecraft:item", 921),
            ("burning_named_stand", "minecraft:armor_stand", 922),
        ] {
            add_entity_with_data(game, id, kind, [5.5, 64.0, 5.5], 0.0, 0.0, 0);
            match kind {
                "minecraft:creeper" => {
                    game.mobs
                        .iter_mut()
                        .find(|m| m.id == id as u32)
                        .unwrap()
                        .fire_ticks = 40;
                }
                "minecraft:item" => {
                    game.handle_packet(C::SetEntityData(play::SetEntityData {
                        id,
                        values: vec![
                            (0, serializer::BYTE, EntityDataValue::Byte(1)),
                            (
                                8,
                                serializer::ITEM_STACK,
                                EntityDataValue::Item(Some(WireStack::new(stone_item, 1))),
                            ),
                        ],
                    }))
                    .unwrap();
                    if let Some(i) = game.dropped.iter().position(|d| d.id == id as u32) {
                        game.dropped.update(i, |d| d.fire_ticks = 40);
                    }
                }
                "minecraft:armor_stand" => {
                    game.handle_packet(C::SetEntityData(play::SetEntityData {
                        id,
                        values: vec![
                            (0, serializer::BYTE, EntityDataValue::Byte(1)),
                            (
                                2,
                                serializer::OPTIONAL_COMPONENT,
                                EntityDataValue::OptText(Some(Tag::String("StandTag".into()))),
                            ),
                            (3, serializer::BOOLEAN, EntityDataValue::Bool(true)),
                        ],
                    }))
                    .unwrap();
                }
                _ => {}
            }
            game.handle_packet(C::SetCamera(play::SetCamera { camera_id: id }))
                .unwrap();

            game.camera_type = CameraType::ThirdPersonBack;
            let mut add_tp = Vec::new();
            let (tp_verts, _, _, _) = game.entity_geometry(cam, 1.0, None, Vec::new(), &mut add_tp);
            if tp_verts.is_empty() && game.see_through_vertices.is_empty() {
                failures.push(format!("{label}: third person drew nothing"));
            }

            game.camera_type = CameraType::FirstPerson;
            let mut add_fp = Vec::new();
            let (fp_verts, _, _, _) = game.entity_geometry(cam, 1.0, None, Vec::new(), &mut add_fp);
            if !fp_verts.is_empty() || !add_fp.is_empty() || !game.see_through_vertices.is_empty() {
                failures.push(format!(
                    "{label}: first person leaked geometry (verts={}, additive={}, see_through={})",
                    fp_verts.len(),
                    add_fp.len(),
                    game.see_through_vertices.len(),
                ));
            }

            game.handle_packet(C::RemoveEntities(play::RemoveEntities { ids: vec![id] }))
                .unwrap();
        }
    });

    assert!(
        failures.is_empty(),
        "{} visibility mismatches against real 26.3 LevelExtractor:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Addendum (b): a spectated entity (mob or non-living) riding a living vehicle — including a
/// scaled ArmorStand or scale-sanitized HappyGhast — uses the vehicle's sanitized scale and
/// camera distance in third person, and a scaled ArmorStand camera uses its own scale & distance.
#[test]
#[ignore = "daily"]
fn spectated_rider_mount_attributes_26_3() {
    use sunrise_client::camera::{Camera, CameraType};

    let witness = parity_witness();
    let cases = witness["spectated_rider_mount_attributes"]["cases"]
        .as_array()
        .unwrap();
    let mut failures = Vec::new();

    with_client_game(|game| {
        let _server = setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
        let scale_id = game
            .motion
            .attributes
            .id("minecraft:scale")
            .unwrap()
            .registry_id();
        let dist_id = game
            .motion
            .attributes
            .id("minecraft:camera_distance")
            .unwrap()
            .registry_id();

        let mut next_id = 950i32;
        for tc in cases {
            let name = tc["name"].as_str().unwrap();
            let rider_kind = tc["rider_kind"].as_str().unwrap();
            let rider_scale = f64::from_bits(hex_f64(&tc["rider_scale_bits"]));
            let rider_dist = f64::from_bits(hex_f64(&tc["rider_distance_bits"]));
            let vehicle_kind = tc["vehicle_kind"].as_str();
            let vehicle_scale = f64::from_bits(hex_f64(&tc["vehicle_scale_bits"]));
            let vehicle_dist = f64::from_bits(hex_f64(&tc["vehicle_distance_bits"]));

            let rider_id = next_id;
            next_id += 1;
            let at = [10.25, 64.0, -3.75];
            add_entity_with_data(game, rider_id, rider_kind, at, 45.0, 22.5, 0);
            if rider_kind != "minecraft:oak_boat" {
                game.handle_packet(C::UpdateAttributes(play::UpdateAttributes {
                    id: rider_id,
                    values: vec![
                        play::AttributeSnapshot {
                            attribute: scale_id,
                            base: rider_scale,
                            modifiers: vec![],
                        },
                        play::AttributeSnapshot {
                            attribute: dist_id,
                            base: rider_dist,
                            modifiers: vec![],
                        },
                    ],
                }))
                .unwrap();
            }

            let mut vehicle_id = None;
            if let Some(vk) = vehicle_kind {
                let vid = next_id;
                next_id += 1;
                vehicle_id = Some(vid);
                add_entity_with_data(game, vid, vk, at, 0.0, 0.0, 0);
                game.handle_packet(C::UpdateAttributes(play::UpdateAttributes {
                    id: vid,
                    values: vec![
                        play::AttributeSnapshot {
                            attribute: scale_id,
                            base: vehicle_scale,
                            modifiers: vec![],
                        },
                        play::AttributeSnapshot {
                            attribute: dist_id,
                            base: vehicle_dist,
                            modifiers: vec![],
                        },
                    ],
                }))
                .unwrap();
                game.handle_packet(C::SetPassengers(play::SetPassengers {
                    vehicle: vid,
                    passengers: vec![rider_id],
                }))
                .unwrap();
            }
            game.refresh_remote_attributes();
            game.handle_packet(C::SetCamera(play::SetCamera {
                camera_id: rider_id,
            }))
            .unwrap();

            // Pin exact unquantized yaw/pitch to match the oracle's (45.0, 22.5).
            let pos = Vec3::new(at[0], at[1], at[2]);
            if let Some(m) = game.mobs.iter_mut().find(|m| m.id == rider_id as u32) {
                (m.position, m.previous) = (pos, pos);
                (m.yaw, m.previous_yaw, m.head_yaw, m.previous_head_yaw) = (45.0, 45.0, 45.0, 45.0);
                (m.head_pitch, m.previous_head_pitch) = (22.5, 22.5);
            }
            if let Some(a) = game
                .armor_stands
                .iter_mut()
                .find(|a| a.id == rider_id as u32)
            {
                (a.position, a.previous) = (pos, pos);
                (a.yaw, a.previous_yaw, a.pitch) = (45.0, 45.0, 22.5);
            }
            game.boats.for_each_mut(|b| {
                if b.id == rider_id as u32 {
                    (b.position, b.previous, b.yaw, b.previous_yaw, b.pitch) =
                        (pos, pos, 45.0, 45.0, 22.5);
                }
            });

            let snap = game
                .spectated_camera_snapshot()
                .unwrap_or_else(|| panic!("{name}: missing spectated camera snapshot"));
            let want_eye = hex_f32(&tc["rider_eye_height_bits"]);
            if snap.eye_height.to_bits() != want_eye {
                failures.push(format!(
                    "{name}: eye_height {:08x} ({}) want {:08x} ({})",
                    snap.eye_height.to_bits(),
                    snap.eye_height,
                    want_eye,
                    f32::from_bits(want_eye)
                ));
            }

            for (ct_name, ct) in [
                ("first_person", CameraType::FirstPerson),
                ("third_person_back", CameraType::ThirdPersonBack),
            ] {
                let mut cam = Camera::default();
                for _ in 0..40 {
                    cam.tick(Some(snap.eye_height), 1.0);
                }
                cam.align_with_entity(&snap, 1.0, ct, |_, _| None);
                let want = &tc[ct_name];
                let want_pos = vec3_bits_expected(&want["pos_bits"]);
                let got_pos = cam.position.map(f64::to_bits);
                if got_pos != want_pos {
                    failures.push(format!(
                        "{name} {ct_name}: pos {:?} want {:?}",
                        cam.position,
                        want_pos.map(f64::from_bits)
                    ));
                }
                if cam.x_rot.to_bits() != hex_f32(&want["xRot_bits"])
                    || cam.y_rot.to_bits() != hex_f32(&want["yRot_bits"])
                    || cam.detached != want["detached"].as_bool().unwrap()
                {
                    failures.push(format!(
                        "{name} {ct_name}: rot/detached mismatch ({}, {}, {})",
                        cam.x_rot, cam.y_rot, cam.detached
                    ));
                }
            }

            let mut to_remove = vec![rider_id];
            if let Some(vid) = vehicle_id {
                to_remove.push(vid);
            }
            game.handle_packet(C::RemoveEntities(play::RemoveEntities { ids: to_remove }))
                .unwrap();
        }
    });

    assert!(
        failures.is_empty(),
        "{} mount attribute mismatches against real 26.3:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
