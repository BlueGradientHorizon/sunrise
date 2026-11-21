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
    screen_effects::Blend,
};
use sunrise_core::geometry::Vec3;
use sunrise_protocol::{
    buf::pack_degrees,
    connection::Pipe,
    entity_data::Value as EntityDataValue,
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

fn setup_remote_overworld(game: &mut Game<'_>, player_id: i32, player_pos: [f64; 3], flat: bool) {
    game.native_lifecycle = NativeLifecycle::default();
    let (client, _server) = Pipe::pair();
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
        setup_remote_overworld(game, player_id, player_pos, false);
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
        setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
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
        setup_remote_overworld(game, 1, [0.0, 64.0, 0.0], true);

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
            setup_remote_overworld(game, 7, [0.5, 64.0, 0.5], true);
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
        setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], false);

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
    with_client_game(|game| {
        setup_remote_overworld(game, 1, [0.5, 64.0, 0.5], true);
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

        // 1. Streaming center follows camera entity.
        game.update_world();
        assert_eq!(
            game.streaming.center,
            (6, 6),
            "streaming.center must follow camera"
        );
        assert_eq!(
            game.streaming.center_y, 300,
            "streaming.center_y must follow camera"
        );

        // 2. Particle camera follows camera entity.
        game.tick_particles();
        assert_eq!(
            game.particles.camera[0].to_bits(),
            96.5f64.to_bits(),
            "particles.camera.x must follow camera"
        );
        assert!(
            game.particles.camera[1] > 300.0,
            "particles.camera.y must follow camera eye, got {}",
            game.particles.camera[1]
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
            joined.contains("96.500 / 300.00000 / 96.500"),
            "F3 position must show camera entity, got:\n{joined}"
        );
        assert!(
            joined.contains("Biome: minecraft:desert"),
            "F3 biome must sample camera entity, got:\n{joined}"
        );
        assert!(
            joined.contains("Client Light: 0"),
            "F3 light must sample camera entity, got:\n{joined}"
        );

        // 4. F3 gizmos: chunk_borders on camera chunk; hitboxes hide camera and show player in 1st person.
        game.debug_entries
            .set_status("chunk_borders", Status::AlwaysOn);
        game.debug_entries
            .set_status("entity_hitboxes", Status::Never);
        let border_gizmos = game.debug_gizmos(1.0, [96.5, 301.62, 96.5]);
        assert!(
            border_gizmos
                .lines
                .iter()
                .any(|l| l.start[0] >= 96.0 && l.start[0] <= 112.0 && l.start[2] >= 96.0),
            "chunk_borders must surround camera chunk (6, 6)"
        );

        game.debug_entries
            .set_status("chunk_borders", Status::Never);
        game.debug_entries
            .set_status("entity_hitboxes", Status::AlwaysOn);
        let hitbox_gizmos = game.debug_gizmos(1.0, [96.5, 301.62, 96.5]);
        let has_player_box = hitbox_gizmos.lines.iter().any(|l| {
            l.start[0] >= 0.0 && l.start[0] <= 1.0 && l.start[1] >= 64.0 && l.start[1] <= 66.0
        });
        assert!(
            has_player_box,
            "local player hitbox must render in FirstPerson when spectating another entity"
        );
        let has_camera_interaction_box = hitbox_gizmos.lines.iter().any(|l| {
            l.start[0] >= 96.4 && l.start[0] <= 96.6 && l.start[1] >= 300.0 && l.start[1] <= 302.0
        });
        assert!(
            !has_camera_interaction_box,
            "camera entity hitbox must be hidden in FirstPerson"
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
        assert!(
            game.spectator_crosshair(),
            "unobstructed chest minecart shows spectator crosshair"
        );
        add_entity(
            game,
            41,
            "minecraft:interaction",
            [0.5, 65.0, 2.0],
            0.0,
            0.0,
        );
        assert!(
            !game.spectator_crosshair(),
            "nearer Interaction is not a MenuProvider and must occlude chest minecart"
        );
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
        setup_remote_overworld(game, 1, [0.99, 64.0, 0.5], true);
        game.world
            .chunks
            .insert((0, 0), Arc::new(make_chunk(0, 0, 0, 0xF0)));
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        game.world.set(1, 65, 0, stone).unwrap();

        game.yaw = 0.0;
        game.pitch = 0.0;
        game.settings.fov = 70.0;
        let hand_proj = sunrise_client::camera::hand_projection(70.0, 1280.0, 800.0, 512.0);
        for _ in 0..20 {
            game.tick_fov();
        }
        let baseline_quads = game.screen_quads(1.0, [1280, 800], 1.0, hand_proj);
        assert!(
            baseline_quads
                .iter()
                .any(|q| q.blend == Blend::Alpha && !q.world_tint),
            "baseline 70-deg view must detect stone at x=1.0"
        );

        game.inventory[0] = Some(ItemStack::new("minecraft:spyglass", 1));
        game.selected = 0;
        game.using = Some((0, 5));
        for _ in 0..20 {
            game.tick_fov();
        }
        let eye = [0.99, 65.62, 0.5];
        assert!(
            game.fov_degrees(1.0, eye) < 15.0,
            "spyglass must narrow render FOV well below option FOV 70"
        );

        let quads = game.screen_quads(1.0, [1280, 800], 1.0, hand_proj);
        assert!(
            quads
                .iter()
                .any(|q| q.blend == Blend::Alpha && !q.world_tint),
            "cull near plane must use max(fov, option_fov)=70 so stone at x=1.0 is view-blocking"
        );
    });
}
