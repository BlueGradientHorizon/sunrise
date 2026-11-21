//! Minecraft 26.3 display parity tests (GenPvP menu TDD red phase).
//! Expected values come strictly from real vanilla 26.3 runs on JDK 25:
//! `tools/witness/26.3/display_parity/witness.json` and `tools/witness/26.3/genpvp_menu/settled.json`.
use super::{RemoteTextDisplay, Shows};
use crate::game::Game;
use crate::game::native_entity_refs::tests::{run_fresh_cases, with_client_game};
use crate::game::native_hooks::particle_count_tests::frame_scene;
use glam::{DVec3, Mat4, Vec3};
use std::path::Path;
use sunrise_client::{
    camera::CameraType,
    gfx::{
        Frustum, JoinedFrusta, OIT_ENTITY, TEXT_POLYGON_OFFSET_OVERLAY, TRANSLUCENT_ITEM_OVERLAY,
    },
    items::ItemStack,
    mesher::{SHADED_CULL_W, SectionMesh, Vertex},
};
use sunrise_core::nbt::{Compound, Tag};
use sunrise_protocol::entity_data::Value;

const DISPLAY_PARITY_WITNESS_26_3: &str =
    include_str!("../../../../../../tools/witness/26.3/display_parity/witness.json");

fn parity_witness() -> serde_json::Value {
    let w: serde_json::Value = serde_json::from_str(DISPLAY_PARITY_WITNESS_26_3).unwrap();
    assert_eq!(w["meta"]["game_version"], "26.3");
    w
}

/// Loads the menu witness recorded on the integration branch at runtime so this branch compiles standalone.
fn genpvp_menu_witness() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/witness/26.3/genpvp_menu/settled.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing genpvp_menu 26.3 witness at {}: {e}",
            path.display()
        )
    });
    let w: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(w["meta"]["game_version"], "26.3");
    w
}

fn hex_u32(v: &serde_json::Value) -> u32 {
    u32::from_str_radix(v.as_str().unwrap(), 16).unwrap()
}

fn hex_u64(v: &serde_json::Value) -> u64 {
    u64::from_str_radix(v.as_str().unwrap(), 16).unwrap()
}

fn hex_f32(v: &serde_json::Value) -> f32 {
    f32::from_bits(hex_u32(v))
}

fn hex_f64(v: &serde_json::Value) -> f64 {
    f64::from_bits(hex_u64(v))
}

fn hex_u32_vec(v: &serde_json::Value) -> Vec<u32> {
    v.as_array().unwrap().iter().map(hex_u32).collect()
}

fn hex_f32_arr<const N: usize>(v: &serde_json::Value) -> [f32; N] {
    let arr = v.as_array().unwrap();
    std::array::from_fn(|i| hex_f32(&arr[i]))
}

fn hex_f64_arr<const N: usize>(v: &serde_json::Value) -> [f64; N] {
    let arr = v.as_array().unwrap();
    std::array::from_fn(|i| hex_f64(&arr[i]))
}

fn billboard_id(name: &str) -> i8 {
    match name {
        "vertical" => 1,
        "horizontal" => 2,
        "center" => 3,
        _ => 0,
    }
}

fn looking_south_frustum(fov_deg: f32, aspect: f32) -> JoinedFrusta {
    let proj = Mat4::perspective_rh(fov_deg.to_radians(), aspect, 256.0, 0.05);
    let view = sunrise_client::camera::view_matrix(0.0, 0.0);
    JoinedFrusta::new(Frustum::from_view_projection(proj * view), [])
}

fn apply_witness_display(game: &mut Game<'_>, entity: &serde_json::Value) -> i32 {
    let id = entity["id"].as_i64().unwrap() as i32;
    let kind = entity["type"].as_str().unwrap();
    let pos = hex_f64_arr::<3>(&entity["pos_bits"]);
    let yaw = hex_f32(&entity["yRot_bits"]);
    let pitch = hex_f32(&entity["xRot_bits"]);
    assert!(game.spawn_remote_object(id, kind, pos, (yaw, pitch), 0));
    let disp = &entity["display"];
    let tx = &disp["transformation"];
    let mut values: Vec<(u8, i32, Value)> = vec![
        (
            11,
            0,
            Value::Floats(hex_f32_arr::<3>(&tx["translation_bits"]).to_vec()),
        ),
        (
            12,
            0,
            Value::Floats(hex_f32_arr::<3>(&tx["scale_bits"]).to_vec()),
        ),
        (
            13,
            0,
            Value::Floats(hex_f32_arr::<4>(&tx["left_rotation_bits"]).to_vec()),
        ),
        (
            14,
            0,
            Value::Floats(hex_f32_arr::<4>(&tx["right_rotation_bits"]).to_vec()),
        ),
        (
            15,
            0,
            Value::Byte(billboard_id(disp["billboard"].as_str().unwrap())),
        ),
        (
            16,
            1,
            Value::Int(disp["brightness_override"].as_i64().unwrap_or(-1) as i32),
        ),
        (17, 2, Value::Float(hex_f32(&disp["view_range_bits"]))),
        (18, 2, Value::Float(hex_f32(&disp["shadow_radius_bits"]))),
        (19, 2, Value::Float(hex_f32(&disp["shadow_strength_bits"]))),
        (20, 2, Value::Float(hex_f32(&disp["width_bits"]))),
        (21, 2, Value::Float(hex_f32(&disp["height_bits"]))),
        (
            22,
            1,
            Value::Int(disp["glow_color"].as_i64().unwrap() as i32),
        ),
    ];
    let block_state = disp["block_state"]
        .as_str()
        .and_then(|name| game.pack.assets.registry.default_state(name));
    if let Some(text) = disp["text"].as_object() {
        let tag = sunrise_core::json_nbt::parse_json_tag(text["json"].as_str().unwrap()).unwrap();
        values.push((23, 0, Value::Text(tag)));
        values.push((
            24,
            1,
            Value::Int(text["line_width"].as_i64().unwrap() as i32),
        ));
        values.push((
            25,
            1,
            Value::Int(text["background"].as_i64().unwrap() as i32),
        ));
        values.push((
            26,
            0,
            Value::Byte(text["text_opacity"].as_i64().unwrap() as i8),
        ));
        values.push((27, 0, Value::Byte(text["flags"].as_i64().unwrap() as i8)));
    }
    let display = game.text_displays.get_mut(&id).unwrap();
    if let Some(state) = block_state {
        display.show(&[(23, 0, Value::Id(state.0 as i32))], None);
    }
    display.metadata(&values);
    id
}

#[test]
fn initial_start_min_value_and_duration_edge_cases_match_vanilla_26_3() {
    let w = parity_witness();
    let lc = &w["lifecycle"];

    // Initial interpolationStartClientTick = Integer.MIN_VALUE: duration=10 without id 8 snaps.
    let min_case = &lc["initial_start_min_value"];
    let mut d = RemoteTextDisplay::new(Shows::Text, [0.0; 3], 0.0);
    d.metadata(&[
        (23, 0, Value::Text(Tag::String("A".into()))),
        (11, 0, Value::Floats(vec![0.0, 0.0, 0.0])),
    ]);
    d.tick();
    d.metadata(&[
        (9, 1, Value::Int(10)),
        (11, 0, Value::Floats(vec![10.0, 0.0, 0.0])),
    ]);
    d.tick();
    let p0 = d.progress(0.0);
    let p_half = d.progress(0.5);
    assert_eq!(p0.to_bits(), hex_u32(&min_case["progress_at_0_bits"]));
    assert_eq!(
        p_half.to_bits(),
        hex_u32(&min_case["progress_at_half_bits"])
    );
    let blended = d.shown.blend(&d.from, p0);
    assert_eq!(
        blended.translation.map(f32::to_bits).to_vec(),
        hex_u32_vec(&min_case["translation_at_0_bits"])
    );

    // Negative duration snaps and updates render state cleanly.
    let dur_case = &lc["duration_zero_and_negative"];
    let mut dn = RemoteTextDisplay::new(Shows::Text, [0.0; 3], 0.0);
    dn.metadata(&[
        (23, 0, Value::Text(Tag::String("N".into()))),
        (11, 0, Value::Floats(vec![0.0, 0.0, 0.0])),
    ]);
    dn.tick();
    dn.metadata(&[
        (8, 1, Value::Int(0)),
        (9, 1, Value::Int(-3)),
        (11, 0, Value::Floats(vec![9.0, 0.0, 0.0])),
        (18, 2, Value::Float(2.0)),
    ]);
    dn.tick();
    let p_neg = dn.progress(0.5);
    assert_eq!(
        p_neg.to_bits(),
        hex_u32(&dur_case["neg_duration_progress_bits"])
    );
    let b_neg = dn.shown.blend(&dn.from, p_neg);
    assert_eq!(
        b_neg.translation.map(f32::to_bits).to_vec(),
        hex_u32_vec(&dur_case["neg_duration_translation_bits"])
    );
    assert_eq!(
        b_neg.shadow_radius.to_bits(),
        hex_u32(&dur_case["neg_duration_shadow_radius_bits"])
    );
}

#[test]
fn teleport_duration_unclamped_negative_same_target_and_rotation_match_vanilla_26_3() {
    let w = parity_witness();
    let tp = &w["teleport_duration_and_pos_rot_lerp"];

    // 1. Client synced teleport_duration = 100 is NOT clamped to 59.
    let c100 = &tp["client_unclamped_100"];
    let mut d = RemoteTextDisplay::new(Shows::Text, [0.0; 3], 0.0);
    d.metadata(&[(10, 1, Value::Int(100))]);
    d.move_to([100.0, 0.0, 0.0], Some((0.0, 0.0)));
    for tick in 1..=100 {
        d.tick();
        if matches!(tick, 1 | 2 | 50 | 59 | 100) {
            let key = format!("tick_{tick}_x_bits");
            assert_eq!(
                d.position.x.to_bits(),
                hex_u64(&c100[&key]),
                "unclamped teleport_duration=100 at tick {tick}"
            );
        }
    }

    // 2. NBT readAdditionalSaveData still clamps teleport_duration to 0..59.
    let mut tag = Compound::new();
    tag.insert("teleport_duration".into(), Tag::Int(100));
    let nbt_entries = sunrise_client::text_display::display_data(&Tag::Compound(tag), false);
    let nbt_tp = nbt_entries
        .iter()
        .find_map(|(id, _, v)| match (id, v) {
            (10, Value::Int(n)) => Some(*n),
            _ => None,
        })
        .unwrap();
    assert_eq!(nbt_tp, tp["nbt_clamped_100"].as_i64().unwrap() as i32);

    // 3. Negative teleport_duration does not snap and does not move.
    let neg = &tp["negative_teleport_duration"];
    let mut d_neg = RemoteTextDisplay::new(Shows::Text, [0.0; 3], 0.0);
    d_neg.metadata(&[(10, 1, Value::Int(-5))]);
    d_neg.move_to([10.0, 0.0, 0.0], Some((45.0, 15.0)));
    assert_eq!(
        d_neg.position.x.to_bits(),
        hex_u64(&neg["before_tick_x_bits"])
    );
    d_neg.tick();
    assert_eq!(
        d_neg.position.x.to_bits(),
        hex_u64(&neg["after_tick_x_bits"])
    );

    // 4. Repeating the identical target does not restart remaining steps.
    let same = &tp["same_target_no_restart"];
    let mut d_same = RemoteTextDisplay::new(Shows::Text, [0.0; 3], 0.0);
    d_same.metadata(&[(10, 1, Value::Int(4))]);
    d_same.move_to([4.0, 0.0, 0.0], Some((0.0, 0.0)));
    d_same.tick();
    d_same.tick();
    assert_eq!(
        d_same.position.x.to_bits(),
        hex_u64(&same["x_after_2_ticks_bits"])
    );
    d_same.move_to([4.0, 0.0, 0.0], Some((0.0, 0.0)));
    d_same.tick();
    assert_eq!(
        d_same.position.x.to_bits(),
        hex_u64(&same["x_after_repeat_and_tick3_bits"])
    );

    // 5. Yaw wraps modulo 360 (`setYRot`), pitch clamps to [-90, 90] (`setXRot`).
    let rot = &tp["rotation_lerp_and_clamp"];
    let mut d_rot = RemoteTextDisplay::new(Shows::Text, [0.0; 3], 350.0);
    d_rot.move_to([0.0; 3], Some((350.0, -80.0)));
    d_rot.metadata(&[(10, 1, Value::Int(2))]);
    d_rot.move_to([0.0; 3], Some((10.0, 80.0)));
    d_rot.tick();
    assert_eq!(
        d_rot.rotation[0].to_bits(),
        hex_u32(&rot["step1_yRot_bits"])
    );
    assert_eq!(
        d_rot.rotation[1].to_bits(),
        hex_u32(&rot["step1_xRot_bits"])
    );
    d_rot.metadata(&[(10, 1, Value::Int(0))]);
    d_rot.move_to([0.0; 3], Some((0.0, 120.0)));
    assert_eq!(
        d_rot.rotation[1].to_bits(),
        hex_u32(&rot["setXRot_120_bits"])
    );
    d_rot.move_to([0.0; 3], Some((0.0, -120.0)));
    assert_eq!(
        d_rot.rotation[1].to_bits(),
        hex_u32(&rot["setXRot_neg120_bits"])
    );
}

#[test]
fn display_shadow_respects_pre_tick_and_invisibility_flag_26_3() {
    let w = parity_witness();
    let sh = &w["display_entity_features"]["invisibility_shadow"];
    let mut d = RemoteTextDisplay::new(Shows::Text, [0.5, 0.0, 0.5], 0.0);
    d.metadata(&[
        (18, 2, Value::Float(2.0)),
        (19, 2, Value::Float(1.0)),
        (23, 0, Value::Text(Tag::String("S".into()))),
    ]);
    assert_eq!(
        d.shadow(1.0)
            .map_or(0.0f32.to_bits(), |(_, r, _)| r.to_bits()),
        hex_u32(&sh["pre_tick_shadow_radius_bits"])
    );
    d.tick();
    assert_eq!(
        d.shadow(1.0)
            .map_or(0.0f32.to_bits(), |(_, r, _)| r.to_bits()),
        hex_u32(&sh["visible_shadow_radius_bits"])
    );
    d.metadata(&[(0, 0, Value::Byte(0x20))]);
    assert_eq!(
        d.shadow(1.0)
            .map_or(0.0f32.to_bits(), |(_, r, _)| r.to_bits()),
        hex_u32(&sh["invisible_shadow_radius_bits"])
    );
}

#[test]
#[ignore = "daily"]
fn display_camera_angles_and_billboard_use_active_camera_not_player_26_3() {
    let w = parity_witness();
    let menu = genpvp_menu_witness();
    with_client_game(|game| {
        // Player looks away while active render camera (spectated entity) uses witnessed angles.
        game.yaw = 123.0;
        game.pitch = 45.0;
        for case in w["billboard_camera_angles"]["cases"].as_array().unwrap() {
            game.camera_type = match case["camera_type"].as_str().unwrap() {
                "THIRD_PERSON_BACK" => CameraType::ThirdPersonBack,
                "THIRD_PERSON_FRONT" => CameraType::ThirdPersonFront,
                _ => CameraType::FirstPerson,
            };
            let cam_yaw = hex_f32(&case["camera_yRot_bits"]);
            let cam_pitch = hex_f32(&case["camera_xRot_bits"]);
            game.posed_rotation = Some((cam_yaw, cam_pitch));
            game.view.y_rot = cam_yaw;
            game.view.x_rot = cam_pitch;
            let got = game.display_camera_angles();
            assert_eq!(
                [got[0].to_bits(), got[1].to_bits()],
                [
                    hex_u32(&case["camera_yRot_bits"]),
                    hex_u32(&case["camera_xRot_bits"]),
                ],
                "camera angles for {} ({})",
                case["name"],
                case["camera_type"]
            );
        }

        // Verify a real GenPvP menu block display (EID 83083) renders with the spectated camera pose.
        game.text_displays.clear();
        game.view_frustum = None;
        let cam = &menu["camera"];
        let cam_pos = DVec3::from_array(hex_f64_arr::<3>(&cam["camera_pos_bits"]));
        let cam_yaw = hex_f32(&cam["camera_yRot_bits"]);
        let cam_pitch = hex_f32(&cam["camera_xRot_bits"]);
        game.camera_type = CameraType::FirstPerson;
        game.posed_rotation = Some((cam_yaw, cam_pitch));
        game.view.y_rot = cam_yaw;
        game.view.x_rot = cam_pitch;
        game.yaw = 123.0;
        game.pitch = 45.0;

        let e83083 = menu["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == 83083)
            .unwrap();
        apply_witness_display(game, e83083);
        game.tick_text_displays();
        let mut out = Vec::new();
        game.block_item_display_geometry(cam_pos, 1.0, &mut out);
        assert!(!out.is_empty());

        let expected_pose =
            Mat4::from_cols_array(&hex_f32_arr::<16>(&e83083["display"]["display_pose_bits"]));
        let expected_corners: Vec<[u32; 3]> = (0..8)
            .map(|bits| {
                let c = Vec3::new(
                    (bits & 1) as f32,
                    ((bits >> 1) & 1) as f32,
                    ((bits >> 2) & 1) as f32,
                );
                expected_pose
                    .transform_point3(c)
                    .to_array()
                    .map(f32::to_bits)
            })
            .collect();
        for v in &out {
            let bits = v.pos.map(f32::to_bits);
            assert!(
                expected_corners.contains(&bits),
                "vertex {:?} ({bits:08x?}) does not match vanilla display_pose_bits corners",
                v.pos
            );
        }
    });
}

#[test]
#[ignore = "daily"]
fn no_culling_text_displays_are_not_frustum_culled_26_3() {
    let w = parity_witness();
    let live = &w["live_culling_and_no_culling"];
    let menu = genpvp_menu_witness();
    with_client_game(|game| {
        let cam = DVec3::new(0.5, 301.620015, 0.5);
        game.camera_type = CameraType::FirstPerson;
        game.yaw = 0.0;
        game.pitch = 0.0;
        game.posed_rotation = Some((0.0, 0.0));
        game.view.y_rot = 0.0;
        game.view.x_rot = 0.0;
        game.view_frustum = Some(looking_south_frustum(70.0, 1280.0 / 800.0));

        // width == 0 && height == 0 (noCulling): translation=(50,0,-50) behind camera still renders.
        assert!(game.spawn_remote_object(
            1001,
            "minecraft:text_display",
            [0.5, 301.62, 0.5],
            (0.0, 0.0),
            0,
        ));
        game.text_displays.get_mut(&1001).unwrap().metadata(&[
            (11, 0, Value::Floats(vec![50.0, 0.0, -50.0])),
            (17, 2, Value::Float(0.03)),
            (23, 0, Value::Text(Tag::String("Menu".into()))),
        ]);
        game.tick_text_displays();
        let (mut normal, mut see_through) = (Vec::new(), Vec::new());
        game.text_display_geometry(cam, 1.0, &mut normal, &mut see_through);
        assert_eq!(
            !normal.is_empty(),
            live["no_culling_outside_frustum_should_render"]
                .as_bool()
                .unwrap()
        );

        // Finite culling box behind the camera must be frustum-culled.
        game.text_displays.clear();
        assert!(game.spawn_remote_object(
            1002,
            "minecraft:text_display",
            [0.5, 301.62, -1.0],
            (0.0, 0.0),
            0,
        ));
        game.text_displays.get_mut(&1002).unwrap().metadata(&[
            (17, 2, Value::Float(0.03)),
            (20, 2, Value::Float(0.2)),
            (21, 2, Value::Float(0.2)),
            (23, 0, Value::Text(Tag::String("Behind".into()))),
        ]);
        game.tick_text_displays();
        normal.clear();
        see_through.clear();
        game.text_display_geometry(cam, 1.0, &mut normal, &mut see_through);
        assert_eq!(
            !normal.is_empty(),
            live["finite_box_behind_camera_should_render"]
                .as_bool()
                .unwrap()
        );

        // Every witnessed GenPvP menu text_display with should_render == true must emit geometry.
        let menu_cam = DVec3::from_array(hex_f64_arr::<3>(&menu["camera"]["camera_pos_bits"]));
        let fov = hex_f32(&menu["frame"]["fov_bits"]);
        game.view_frustum = Some(looking_south_frustum(fov, 1280.0 / 800.0));
        let mut checked = 0usize;
        for entity in menu["entities"].as_array().unwrap() {
            if entity["type"] != "minecraft:text_display" || entity["should_render"] != true {
                continue;
            }
            game.text_displays.clear();
            let id = apply_witness_display(game, entity);
            game.tick_text_displays();
            normal.clear();
            see_through.clear();
            game.text_display_geometry(menu_cam, 1.0, &mut normal, &mut see_through);
            assert!(
                !normal.is_empty() || !see_through.is_empty(),
                "GenPvP menu text_display EID {id} has should_render=true in vanilla 26.3 but emitted 0 vertices"
            );
            checked += 1;
        }
        assert_eq!(checked, 145);
    });
}

#[test]
#[ignore = "daily"]
fn display_lifecycle_pre_tick_deferred_swap_live_culling_and_culled_progress_26_3() {
    let w = parity_witness();
    let lc = &w["lifecycle"];
    let live = &w["live_culling_and_no_culling"];
    with_client_game(|game| {
        game.view_frustum = None;
        game.posed_rotation = Some((0.0, 0.0));
        game.view.y_rot = 0.0;
        game.view.x_rot = 0.0;
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        let blue_terra = game
            .pack
            .assets
            .registry
            .default_state("minecraft:blue_terracotta")
            .unwrap();

        // 1. BlockDisplay: no rendering pre-tick or after empty tick; first metadata tick snaps; swap defers to tick.
        let bd_case = &lc["block_display_pre_tick_and_swap"];
        assert!(game.spawn_remote_object(2001, "minecraft:block_display", [0.0; 3], (0.0, 0.0), 0));
        let mut out = Vec::new();
        game.block_item_display_geometry(DVec3::ZERO, 1.0, &mut out);
        assert_eq!(
            !out.is_empty(),
            bd_case["on_spawn_render_state_present"].as_bool().unwrap()
        );
        game.tick_text_displays(); // empty tick before metadata
        game.block_item_display_geometry(DVec3::ZERO, 1.0, &mut out);
        assert_eq!(
            !out.is_empty(),
            bd_case["after_empty_tick_render_state_present"]
                .as_bool()
                .unwrap()
        );
        {
            let d = game.text_displays.get_mut(&2001).unwrap();
            d.show(&[(23, 0, Value::Id(stone.0 as i32))], None);
            d.metadata(&[
                (8, 1, Value::Int(0)),
                (9, 1, Value::Int(4)),
                (11, 0, Value::Floats(vec![2.0, 3.0, 4.0])),
                (12, 0, Value::Floats(vec![2.0, 2.0, 2.0])),
            ]);
        }
        game.block_item_display_geometry(DVec3::ZERO, 1.0, &mut out);
        assert_eq!(
            !out.is_empty(),
            bd_case["after_metadata_before_tick_render_state_present"]
                .as_bool()
                .unwrap()
        );
        game.tick_text_displays();
        let mut after_stone = Vec::new();
        game.block_item_display_geometry(DVec3::ZERO, 0.0, &mut after_stone);
        assert_eq!(
            !after_stone.is_empty(),
            bd_case["after_first_meta_tick_render_state_present"]
                .as_bool()
                .unwrap()
        );
        // Snapped to translation (2, 3, 4), NOT interpolated from (0, 0, 0).
        let min_x = after_stone
            .iter()
            .map(|v| v.pos[0])
            .fold(f32::INFINITY, f32::min);
        assert_eq!(
            min_x.to_bits(),
            hex_u32(&bd_case["tick2_translation_at_0_bits"][0])
        );
        // Swap block without ticking: must still render stone until next tick.
        {
            let d = game.text_displays.get_mut(&2001).unwrap();
            d.show(&[(23, 0, Value::Id(blue_terra.0 as i32))], None);
            d.metadata(&[(11, 0, Value::Floats(vec![6.0, 3.0, 4.0]))]);
        }
        let mut before_swap_tick = Vec::new();
        game.block_item_display_geometry(DVec3::ZERO, 0.0, &mut before_swap_tick);
        assert_eq!(
            bytemuck::cast_slice::<Vertex, u8>(&before_swap_tick),
            bytemuck::cast_slice::<Vertex, u8>(&after_stone),
            "block swap must wait for the next display tick"
        );
        game.tick_text_displays();
        let mut after_swap_tick = Vec::new();
        game.block_item_display_geometry(DVec3::ZERO, 0.5, &mut after_swap_tick);
        assert_ne!(
            bytemuck::cast_slice::<Vertex, u8>(&after_swap_tick),
            bytemuck::cast_slice::<Vertex, u8>(&after_stone),
            "block swap must take effect on the tick"
        );

        // 2. ItemDisplay: pre-tick renders nothing; item & context swap defers to tick.
        game.text_displays.clear();
        let id_case = &lc["item_display_pre_tick_and_swap"];
        assert!(game.spawn_remote_object(2002, "minecraft:item_display", [0.0; 3], (0.0, 0.0), 0));
        game.text_displays.get_mut(&2002).unwrap().show(
            &[(24, 0, Value::Byte(6))],
            Some(Some(ItemStack::new("minecraft:end_crystal", 1))),
        );
        out.clear();
        game.block_item_display_geometry(DVec3::ZERO, 1.0, &mut out);
        assert_eq!(
            !out.is_empty(),
            id_case["before_tick_has_item_state"].as_bool().unwrap()
        );
        game.tick_text_displays();
        let mut crystal_verts = Vec::new();
        game.block_item_display_geometry(DVec3::ZERO, 1.0, &mut crystal_verts);
        assert!(!crystal_verts.is_empty());
        game.text_displays.get_mut(&2002).unwrap().show(
            &[(24, 0, Value::Byte(8))],
            Some(Some(ItemStack::new("minecraft:diamond_sword", 1))),
        );
        let mut pre_swap = Vec::new();
        game.block_item_display_geometry(DVec3::ZERO, 1.0, &mut pre_swap);
        assert_eq!(
            bytemuck::cast_slice::<Vertex, u8>(&pre_swap),
            bytemuck::cast_slice::<Vertex, u8>(&crystal_verts),
            "item swap must wait for the next display tick"
        );

        // 3. Live view_range without tick + uninterpolated distance check.
        game.text_displays.clear();
        assert!(game.spawn_remote_object(
            2003,
            "minecraft:text_display",
            [0.5, 301.62, 0.5],
            (0.0, 0.0),
            0,
        ));
        game.text_displays.get_mut(&2003).unwrap().metadata(&[(
            23,
            0,
            Value::Text(Tag::String("VR".into())),
        )]);
        game.tick_text_displays();
        // Standalone view_range update (no render state id, no tick): range = 1.92.
        game.text_displays
            .get_mut(&2003)
            .unwrap()
            .metadata(&[(17, 2, Value::Float(0.03))]);
        let (mut normal, mut see_through) = (Vec::new(), Vec::new());
        game.text_display_geometry(
            DVec3::new(0.5 + 1.91, 301.62, 0.5),
            1.0,
            &mut normal,
            &mut see_through,
        );
        assert_eq!(
            !normal.is_empty(),
            live["live_vr_at_1_91"].as_bool().unwrap()
        );
        normal.clear();
        game.text_display_geometry(
            DVec3::new(0.5 + 1.93, 301.62, 0.5),
            1.0,
            &mut normal,
            &mut see_through,
        );
        assert_eq!(
            !normal.is_empty(),
            live["live_vr_at_1_93"].as_bool().unwrap()
        );

        // Moving display: distance uses uninterpolated `position`, not `previous.lerp(position, 0.0)`.
        {
            let d = game.text_displays.get_mut(&2003).unwrap();
            d.previous = DVec3::new(0.5, 301.62, 0.5);
            d.position = DVec3::new(2.5, 301.62, 0.5);
        }
        normal.clear();
        game.text_display_geometry(
            DVec3::new(0.5, 301.62, 0.5),
            0.0,
            &mut normal,
            &mut see_through,
        );
        assert_eq!(
            !normal.is_empty(),
            live["moving_uninterpolated_outside_range_partial_0"]
                .as_bool()
                .unwrap()
        );
        {
            let d = game.text_displays.get_mut(&2003).unwrap();
            d.previous = DVec3::new(2.5, 301.62, 0.5);
            d.position = DVec3::new(0.5, 301.62, 0.5);
        }
        normal.clear();
        game.text_display_geometry(
            DVec3::new(0.5, 301.62, 0.5),
            0.0,
            &mut normal,
            &mut see_through,
        );
        assert_eq!(
            !normal.is_empty(),
            live["moving_uninterpolated_inside_range_partial_0"]
                .as_bool()
                .unwrap()
        );

        // 4. last_progress must NOT advance during frames where the display is culled.
        game.text_displays.clear();
        let culled_case = &lc["last_progress_while_culled"];
        assert!(game.spawn_remote_object(2004, "minecraft:text_display", [0.0; 3], (0.0, 0.0), 0));
        game.text_displays.get_mut(&2004).unwrap().metadata(&[
            (23, 0, Value::Text(Tag::String("C".into()))),
            (11, 0, Value::Floats(vec![0.0, 0.0, 0.0])),
            (17, 2, Value::Float(0.05)), // range = 3.2
        ]);
        game.tick_text_displays(); // tick 1
        game.text_displays.get_mut(&2004).unwrap().metadata(&[
            (8, 1, Value::Int(0)),
            (9, 1, Value::Int(4)),
            (11, 0, Value::Floats(vec![10.0, 0.0, 0.0])),
        ]);
        game.tick_text_displays(); // tick 2
        game.tick_text_displays(); // tick 3: progress(0.0) = 0.25
        normal.clear();
        game.text_display_geometry(DVec3::ZERO, 0.0, &mut normal, &mut see_through);
        assert!(!normal.is_empty());
        assert_eq!(
            game.text_displays[&2004].last_progress.to_bits(),
            hex_u32(&culled_case["visible_progress_bits"])
        );
        game.tick_text_displays(); // tick 4: culled (camera 100 blocks away)
        normal.clear();
        game.text_display_geometry(
            DVec3::new(100.0, 0.0, 0.0),
            0.0,
            &mut normal,
            &mut see_through,
        );
        assert!(normal.is_empty());
        // Tick 5: retarget to 20; must start from 0.25 * 10 = 2.5, not 7.5.
        game.text_displays.get_mut(&2004).unwrap().metadata(&[
            (8, 1, Value::Int(0)),
            (9, 1, Value::Int(4)),
            (11, 0, Value::Floats(vec![20.0, 0.0, 0.0])),
        ]);
        game.tick_text_displays();
        let d = &game.text_displays[&2004];
        let p = d.progress(0.0);
        assert_eq!(
            p.to_bits(),
            hex_u32(&culled_case["retarget_progress_0_bits"])
        );
        assert_eq!(
            d.shown
                .blend(&d.from, p)
                .translation
                .map(f32::to_bits)
                .to_vec(),
            hex_u32_vec(&culled_case["retarget_start_translation_bits"])
        );

        // 5. Rendering during duration == 0 must return 1.0 without overwriting last_progress.
        game.text_displays.clear();
        let dur_case = &lc["duration_zero_and_negative"];
        assert!(game.spawn_remote_object(2005, "minecraft:text_display", [0.0; 3], (0.0, 0.0), 0));
        game.text_displays.get_mut(&2005).unwrap().metadata(&[
            (23, 0, Value::Text(Tag::String("Z".into()))),
            (11, 0, Value::Floats(vec![0.0, 0.0, 0.0])),
        ]);
        game.tick_text_displays(); // tick 1
        game.text_displays.get_mut(&2005).unwrap().metadata(&[
            (8, 1, Value::Int(0)),
            (9, 1, Value::Int(4)),
            (11, 0, Value::Floats(vec![8.0, 0.0, 0.0])),
        ]);
        game.tick_text_displays(); // tick 2
        game.tick_text_displays(); // tick 3: progress(0.0) = 0.25
        normal.clear();
        game.text_display_geometry(DVec3::ZERO, 0.0, &mut normal, &mut see_through);
        assert_eq!(
            game.text_displays[&2005].last_progress.to_bits(),
            hex_u32(&dur_case["progress_before_zero_bits"])
        );
        game.text_displays
            .get_mut(&2005)
            .unwrap()
            .metadata(&[(9, 1, Value::Int(0))]);
        game.tick_text_displays(); // tick 4: duration = 0
        normal.clear();
        game.text_display_geometry(DVec3::ZERO, 0.5, &mut normal, &mut see_through);
        game.text_displays.get_mut(&2005).unwrap().metadata(&[
            (8, 1, Value::Int(0)),
            (9, 1, Value::Int(4)),
            (11, 0, Value::Floats(vec![20.0, 0.0, 0.0])),
        ]);
        game.tick_text_displays(); // tick 5: must start from 0.25 * 8 = 2.0, not 8.0
        let dz = &game.text_displays[&2005];
        let p_after = dz.progress(0.0);
        assert_eq!(
            p_after.to_bits(),
            hex_u32(&dur_case["progress_after_zero_bits"])
        );
        assert_eq!(
            dz.shown
                .blend(&dz.from, p_after)
                .translation
                .map(f32::to_bits)
                .to_vec(),
            hex_u32_vec(&dur_case["translation_after_zero_bits"])
        );
    });
}

#[test]
#[ignore = "daily"]
fn text_display_render_order_solid_split_and_see_through_sort_26_3() {
    let w = parity_witness();
    let menu = genpvp_menu_witness();
    let phases = &w["text_submit_phases_and_sorting"];
    with_client_game(|game| {
        game.view_frustum = None;
        game.posed_rotation = Some((0.0, 0.0));
        game.view.y_rot = 0.0;
        game.view.x_rot = 0.0;

        // 1. Deterministic spawn/insertion order matching GenPvP menu `render_order` (including ID inversions).
        game.text_displays.clear();
        let mut menu_texts: Vec<&serde_json::Value> = menu["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["type"] == "minecraft:text_display" && e["render_order"].is_u64())
            .collect();
        menu_texts.sort_by_key(|e| e["render_order"].as_u64().unwrap());
        let mut sample_ids: Vec<i32> = menu_texts
            .iter()
            .take(20)
            .map(|e| e["id"].as_i64().unwrap() as i32)
            .collect();
        // Append the ID-inverted pairs observed in vanilla's EntityLookup.byId insertion order.
        for eid in [83226, 83191, 83242, 83102, 83249, 83232] {
            if !sample_ids.contains(&eid) {
                sample_ids.push(eid);
            }
        }
        let mut expected_bgs = Vec::new();
        for (idx, &eid) in sample_ids.iter().enumerate() {
            assert!(game.spawn_remote_object(
                eid,
                "minecraft:text_display",
                [0.0; 3],
                (0.0, 0.0),
                0
            ));
            let argb = 0xFF00_0001u32 + idx as u32;
            let abgr =
                0xFF00_0000u32 | ((argb & 0xFF) << 16) | (argb & 0xFF00) | ((argb >> 16) & 0xFF);
            expected_bgs.push(abgr);
            game.text_displays.get_mut(&eid).unwrap().metadata(&[
                (23, 0, Value::Text(Tag::String(" ".into()))),
                (25, 1, Value::Int(argb as i32)),
                (26, 0, Value::Byte(-1)),
            ]);
        }
        game.tick_text_displays();
        let (mut normal, mut see_through) = (Vec::new(), Vec::new());
        game.text_display_geometry(DVec3::ZERO, 1.0, &mut normal, &mut see_through);
        let got_bgs: Vec<u32> = normal
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|q| q[0].layer == TEXT_POLYGON_OFFSET_OVERLAY)
            .map(|q| q[0].color)
            .collect();
        assert_eq!(
            got_bgs, expected_bgs,
            "text displays must emit in insertion order matching vanilla EntityLookup.byId"
        );

        // 2. Solid vs translucent phase routing (`canRenderAsSolid` in `phase_routing`).
        let routing = phases["phase_routing"].as_array().unwrap();
        let find_route = |name: &str, improved: bool| {
            routing
                .iter()
                .find(|r| r["name"] == name && r["improved_transparency"] == improved)
                .unwrap()
        };
        let cases: [(&str, &str, u32, i8); 4] = [
            ("opaque_panel", "X", 0xFF12_0A24, -1),
            (
                "translucent_panel_with_opaque_text",
                "1000 ELO",
                0xA010_1722,
                -1,
            ),
            ("transparent_bg_opaque_label", "Kuyruk", 0x0000_0000, -1),
            ("translucent_text_and_bg", "Fade", 0x8000_0000, 128u8 as i8),
        ];
        for (name, text, bg, opacity) in cases {
            game.text_displays.clear();
            assert!(game.spawn_remote_object(
                3001,
                "minecraft:text_display",
                [0.0; 3],
                (0.0, 0.0),
                0
            ));
            game.text_displays.get_mut(&3001).unwrap().metadata(&[
                (23, 0, Value::Text(Tag::String(text.into()))),
                (25, 1, Value::Int(bg as i32)),
                (26, 0, Value::Byte(opacity)),
            ]);
            game.tick_text_displays();
            normal.clear();
            see_through.clear();
            game.text_display_geometry(DVec3::ZERO, 1.0, &mut normal, &mut see_through);
            let quads = normal.as_chunks::<4>().0;
            assert!(!quads.is_empty(), "{name}: emitted no quads");
            let route = find_route(name, false);
            let text_is_solid = route["solid"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "text:POLYGON_OFFSET");
            let bg_is_translucent = route["texts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "background:POLYGON_OFFSET");
            let bg_abgr = (bg & 0xFF00_FF00) | ((bg & 0xFF) << 16) | ((bg >> 16) & 0xFF);
            let mut bg_quads = Vec::new();
            let mut glyph_quads = Vec::new();
            for q in quads {
                if bg != 0 && q[0].color == bg_abgr {
                    bg_quads.push(q[0].layer);
                } else {
                    glyph_quads.push(q[0].layer);
                }
            }
            if bg_is_translucent {
                assert!(!bg_quads.is_empty(), "{name}: missing background quad");
                assert!(
                    bg_quads.iter().all(|&l| l == TEXT_POLYGON_OFFSET_OVERLAY),
                    "{name}: background must stay in translucent/OIT polygon-offset overlay, got {bg_quads:?}"
                );
            } else {
                assert!(bg_quads.is_empty(), "{name}: unexpected background quad");
            }
            assert!(!glyph_quads.is_empty(), "{name}: missing glyph quads");
            if text_is_solid {
                assert!(
                    glyph_quads
                        .iter()
                        .all(|&l| l != TEXT_POLYGON_OFFSET_OVERLAY && (l & OIT_ENTITY) == 0),
                    "{name}: opaque text must route to a solid overlay distinct from TEXT_POLYGON_OFFSET_OVERLAY, got {glyph_quads:?}"
                );
            } else {
                assert!(
                    glyph_quads
                        .iter()
                        .all(|&l| l == TEXT_POLYGON_OFFSET_OVERLAY),
                    "{name}: translucent text must route to TEXT_POLYGON_OFFSET_OVERLAY, got {glyph_quads:?}"
                );
            }
        }

        // 3. See-through per-submit far-to-near sort (stable on ties).
        game.text_displays.clear();
        let st_specs: [(&str, f64, u32); 4] = [
            ("near_2", -2.0, 0xFF00_0001),
            ("tie_first_4", -4.0, 0xFF00_0002),
            ("far_6", -6.0, 0xFF00_0003),
            ("tie_second_4", -4.0, 0xFF00_0004),
        ];
        for (i, &(_, z, argb)) in st_specs.iter().enumerate() {
            let id = 4000 + i as i32;
            assert!(game.spawn_remote_object(
                id,
                "minecraft:text_display",
                [0.0, 0.0, z],
                (0.0, 0.0),
                0
            ));
            game.text_displays.get_mut(&id).unwrap().metadata(&[
                (23, 0, Value::Text(Tag::String("T".into()))),
                (25, 1, Value::Int(argb as i32)),
                (26, 0, Value::Byte(-1)),
                (27, 0, Value::Byte(2)), // see_through
            ]);
        }
        game.tick_text_displays();
        normal.clear();
        see_through.clear();
        game.text_display_geometry(DVec3::ZERO, 1.0, &mut normal, &mut see_through);
        assert!(normal.is_empty());
        let expected_names: Vec<&str> = phases["see_through_sorted_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let got_names: Vec<&str> = see_through
            .as_chunks::<4>()
            .0
            .iter()
            .filter_map(|q| {
                st_specs.iter().find_map(|&(name, _, argb)| {
                    let abgr = (argb & 0xFF00_FF00) | ((argb & 0xFF) << 16) | ((argb >> 16) & 0xFF);
                    (q[0].color == abgr).then_some(name)
                })
            })
            .collect();
        assert_eq!(got_names, expected_names);
        // Per-submit sorting keeps each display's background quad immediately before its glyph quad.
        let st_quads = see_through.as_chunks::<4>().0;
        assert_eq!(st_quads.len(), 8);
        for pair in st_quads.chunks_exact(2) {
            assert_ne!(pair[0][0].color, 0xFFFF_FFFF, "background precedes glyph");
            assert_eq!(
                pair[1][0].color, 0xFFFF_FFFF,
                "glyph follows its background"
            );
        }
    });
}

#[test]
#[ignore = "daily"]
fn text_display_resolves_inline_player_head_profiles_26_3() {
    let menu = genpvp_menu_witness();
    let e83225 = menu["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == 83225)
        .unwrap();
    let json_text = e83225["display"]["text"]["json"].as_str().unwrap();
    let parsed_json: serde_json::Value = serde_json::from_str(json_text).unwrap();
    let profile_json = &parsed_json["extra"][0]["player"];
    let info = sunrise_client::skins::parse_profile(profile_json);
    let id_key = format!("id:{:032x}", info.id);
    let name_key = format!(
        "name:{}",
        profile_json["name"].as_str().unwrap().to_ascii_lowercase()
    );

    with_client_game(|game| {
        game.view_frustum = None;
        game.posed_rotation = Some((0.0, 0.0));
        game.view.y_rot = 0.0;
        game.view.x_rot = 0.0;
        assert!(!game.gui.atlas.object_skins.contains_key(&id_key));
        assert!(!game.gui.atlas.object_skins.contains_key(&name_key));

        apply_witness_display(game, e83225);
        game.tick_text_displays();
        let cam = DVec3::from_array(hex_f64_arr::<3>(&menu["camera"]["camera_pos_bits"]));
        let (mut normal, mut see_through) = (Vec::new(), Vec::new());
        game.text_display_geometry(cam, 1.0, &mut normal, &mut see_through);
        assert!(
            game.gui.atlas.object_skins.contains_key(&id_key)
                && game.gui.atlas.object_skins.contains_key(&name_key),
            "inline object:player profile from EID 83225 must be forwarded to resolve_inline_profiles (keys: {id_key}, {name_key})"
        );
    });
}

#[test]
#[ignore = "daily"]
fn block_display_backface_culling_translucent_pass_and_light_emission_26_3() {
    let w = parity_witness();
    let bd = &w["block_display_render_type_and_cull"];
    with_client_game(|game| {
        game.view_frustum = None;
        game.posed_rotation = Some((0.0, 0.0));
        game.view.y_rot = 0.0;
        game.view.x_rot = 0.0;
        let chunk = sunrise_client::chunk::Chunk::empty(0, 0);
        game.world.chunks.insert((0, 0), std::sync::Arc::new(chunk));

        let emit_block = |game: &mut Game<'_>, id: i32, block: &str| -> Vec<Vertex> {
            game.text_displays.clear();
            let state = game.pack.assets.registry.default_state(block).unwrap();
            assert!(game.spawn_remote_object(
                id,
                "minecraft:block_display",
                [0.5, 64.0, 0.5],
                (0.0, 0.0),
                0,
            ));
            game.text_displays
                .get_mut(&id)
                .unwrap()
                .show(&[(23, 0, Value::Id(state.0 as i32))], None);
            game.tick_text_displays();
            let mut out = Vec::new();
            game.block_item_display_geometry(DVec3::new(0.5, 64.0, 3.5), 1.0, &mut out);
            assert!(!out.is_empty(), "{block} emitted no vertices");
            out
        };

        let concrete = emit_block(game, 5001, "minecraft:black_concrete");
        assert_eq!(bd["cutout_cull"], true);
        assert_eq!(bd["setup_model_opaque_is_cutout_sheet"], true);
        assert!(
            concrete.iter().all(|v| v.normal[3] == SHADED_CULL_W),
            "opaque block display must use back-face culling (SHADED_CULL_W)"
        );
        assert!(concrete.iter().all(|v| (v.layer & 0xffff) == 0));

        let glass = emit_block(game, 5002, "minecraft:blue_stained_glass");
        assert_eq!(bd["translucent_cull"], true);
        assert_eq!(bd["setup_model_translucent_is_translucent_sheet"], true);
        assert!(
            glass.iter().all(|v| v.normal[3] == SHADED_CULL_W),
            "translucent block display must use back-face culling (SHADED_CULL_W)"
        );
        assert!(
            glass
                .iter()
                .all(|v| (v.layer & 0xffff) == TRANSLUCENT_ITEM_OVERLAY),
            "translucent block display must route to TRANSLUCENT_ITEM_OVERLAY"
        );

        let glowstone = emit_block(game, 5003, "minecraft:glowstone");
        let expected_block_light = bd["blocks"]["minecraft:glowstone"]["packed_block_light_coords"]
            .as_u64()
            .unwrap() as u32;
        assert!(
            glowstone
                .iter()
                .all(|v| ((v.light >> 8) & 0xff) == expected_block_light),
            "glowstone block display must emit block light {expected_block_light}, got {}",
            (glowstone[0].light >> 8) & 0xff
        );
    });
}

#[test]
#[ignore = "daily"]
fn display_name_tags_and_fire_block_light_match_vanilla_26_3() {
    let w = parity_witness();
    let feats = &w["display_entity_features"];
    let nt = &feats["name_tags"];
    let fire = &feats["fire"];
    with_client_game(|game| {
        game.view_frustum = None;
        game.posed_rotation = Some((0.0, 0.0));
        game.view.y_rot = 0.0;
        game.view.x_rot = 0.0;
        let chunk = sunrise_client::chunk::Chunk::empty(0, 0);
        game.world.chunks.insert((0, 0), std::sync::Arc::new(chunk));

        // 1. Display name tags.
        assert!(game.spawn_remote_object(6001, "minecraft:text_display", [0.0; 3], (0.0, 0.0), 0));
        game.text_displays.get_mut(&6001).unwrap().metadata(&[
            (2, 0, Value::OptText(Some(Tag::String("DisplayTag".into())))),
            (3, 0, Value::Bool(true)),
        ]);
        // Pre-first-tick: DisplayRenderer.submit does not submit name tag.
        assert_eq!(
            !game.shown_name_tags(DVec3::new(0.0, 0.0, 5.0)).is_empty(),
            nt["pre_tick_submitted_name_tag"].as_bool().unwrap()
        );
        // Post-first-tick with empty text: hasSubState() is true -> submits name tag.
        game.text_displays.get_mut(&6001).unwrap().metadata(&[(
            23,
            0,
            Value::Text(Tag::String("".into())),
        )]);
        game.tick_text_displays();
        let tags = game.shown_name_tags(DVec3::new(0.0, 0.0, 5.0));
        assert_eq!(
            tags.iter().any(|t| t == "DisplayTag"),
            nt["post_tick_empty_text_submitted_name_tag"]
                .as_bool()
                .unwrap()
        );
        // Beyond 64 blocks: suppressed.
        assert_eq!(
            game.shown_name_tags(DVec3::new(0.0, 0.0, 65.0))
                .iter()
                .any(|t| t == "DisplayTag"),
            nt["beyond_64_extracted_name"].as_bool().unwrap()
        );
        // CustomNameVisible = false (not crosshair pick): suppressed.
        game.text_displays
            .get_mut(&6001)
            .unwrap()
            .metadata(&[(3, 0, Value::Bool(false))]);
        assert_eq!(
            game.shown_name_tags(DVec3::new(0.0, 0.0, 5.0))
                .iter()
                .any(|t| t == "DisplayTag"),
            nt["name_visible_false_extracted_name"].as_bool().unwrap()
        );

        // 2. Burning display block light = 15 without brightness override; override wins when set.
        game.text_displays.clear();
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        assert!(game.spawn_remote_object(
            6002,
            "minecraft:block_display",
            [0.5, 64.0, 0.5],
            (0.0, 0.0),
            0,
        ));
        {
            let d = game.text_displays.get_mut(&6002).unwrap();
            d.show(&[(23, 0, Value::Id(stone.0 as i32))], None);
            d.metadata(&[(0, 0, Value::Byte(0x01))]); // FLAG_ONFIRE
        }
        game.tick_text_displays();
        let mut out = Vec::new();
        game.block_item_display_geometry(DVec3::new(0.5, 64.0, 3.5), 1.0, &mut out);
        assert!(!out.is_empty());
        let expected_fire_block =
            fire["block_light_no_override_on_fire"].as_u64().unwrap() as u32 * 16;
        assert_eq!((out[0].light >> 8) & 0xff, expected_fire_block);

        // Brightness override (block=3, sky=5) beats fire light 15.
        game.text_displays.get_mut(&6002).unwrap().metadata(&[(
            16,
            1,
            Value::Int((5 << 20) | (3 << 4)),
        )]);
        game.tick_text_displays();
        out.clear();
        game.block_item_display_geometry(DVec3::new(0.5, 64.0, 3.5), 1.0, &mut out);
        assert!(!out.is_empty());
        let expected_override_block =
            fire["block_light_with_override_on_fire"].as_u64().unwrap() as u32 * 16;
        assert_eq!((out[0].light >> 8) & 0xff, expected_override_block);
    });
}

const SECTION_GATE_NAME: &str = "game::text_displays::display_parity_tests::section_compiled_and_visible_gates_displays_and_mobs_26_3";
const SECTION_GATE_CASE: &str = "SUNRISE_DISPLAY_SECTION_GATE_CASE";

#[test]
#[ignore = "daily"]
fn section_compiled_and_visible_gates_displays_and_mobs_26_3() {
    if run_fresh_cases(SECTION_GATE_NAME, SECTION_GATE_CASE, &["section_gate"]) {
        return;
    }
    let w = parity_witness();
    let gate = &w["section_compiled_and_visible_gate"];
    with_client_game(|game| {
        let _event_loop = frame_scene(game);
        game.view_frustum = None;
        game.posed_rotation = Some((0.0, 0.0));
        game.view.y_rot = 0.0;
        game.view.x_rot = 0.0;
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        let cam = DVec3::new(0.5, 65.5, 0.5);

        // Inside build height (y = 65.5), section not yet compiled -> hidden.
        assert!(game.spawn_remote_object(
            7001,
            "minecraft:text_display",
            [0.5, 65.5, 3.5],
            (0.0, 0.0),
            0,
        ));
        game.text_displays.get_mut(&7001).unwrap().metadata(&[(
            23,
            0,
            Value::Text(Tag::String("In".into())),
        )]);
        assert!(game.spawn_remote_object(
            7002,
            "minecraft:block_display",
            [0.5, 65.5, 3.5],
            (0.0, 0.0),
            0,
        ));
        game.text_displays
            .get_mut(&7002)
            .unwrap()
            .show(&[(23, 0, Value::Id(stone.0 as i32))], None);
        game.tick_text_displays();
        let mob_start = game.mobs.len();
        assert!(
            game.spawn_mob_raw("minecraft:zombie", [0.5, 65.5, 3.5], true)
                .is_some()
        );
        let mob_range = mob_start..game.mobs.len();

        let (mut normal, mut see_through, mut blocks, mut mob_verts) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        game.text_display_geometry(cam, 1.0, &mut normal, &mut see_through);
        game.block_item_display_geometry(cam, 1.0, &mut blocks);
        let mob_frame = game.begin_mob_geometry(cam, 1.0);
        game.mob_run_geometry(&mob_frame, mob_range.clone(), &mut mob_verts);
        let uncompiled = gate["inside_build_uncompiled"].as_bool().unwrap();
        assert_eq!(
            !normal.is_empty(),
            uncompiled,
            "text display in uncompiled section"
        );
        assert_eq!(
            !blocks.is_empty(),
            uncompiled,
            "block display in uncompiled section"
        );
        assert_eq!(
            !mob_verts.is_empty(),
            uncompiled,
            "mob in uncompiled section"
        );

        // Compiled empty section while fading (< 0.3 visibility) stays hidden; fade = 0 renders.
        let sy = game.world.shape.column().section_index(65).unwrap() as i32;
        game.settings.chunk_fade = 10.0;
        game.renderer
            .as_mut()
            .unwrap()
            .upload((0, sy, 0), &SectionMesh::default());
        normal.clear();
        blocks.clear();
        mob_verts.clear();
        game.text_display_geometry(cam, 1.0, &mut normal, &mut see_through);
        game.block_item_display_geometry(cam, 1.0, &mut blocks);
        let mob_frame = game.begin_mob_geometry(cam, 1.0);
        game.mob_run_geometry(&mob_frame, mob_range.clone(), &mut mob_verts);
        let fading = gate["inside_build_empty_fading_0_2"].as_bool().unwrap();
        assert_eq!(
            !normal.is_empty(),
            fading,
            "text display in fading empty section"
        );
        assert_eq!(
            !blocks.is_empty(),
            fading,
            "block display in fading empty section"
        );
        assert_eq!(!mob_verts.is_empty(), fading, "mob in fading empty section");

        game.settings.chunk_fade = 0.0;
        normal.clear();
        blocks.clear();
        mob_verts.clear();
        game.text_display_geometry(cam, 1.0, &mut normal, &mut see_through);
        game.block_item_display_geometry(cam, 1.0, &mut blocks);
        let mob_frame = game.begin_mob_geometry(cam, 1.0);
        game.mob_run_geometry(&mob_frame, mob_range, &mut mob_verts);
        let visible = gate["inside_build_empty_fade_0"].as_bool().unwrap();
        assert_eq!(
            !normal.is_empty(),
            visible,
            "text display in visible empty section"
        );
        assert_eq!(
            !blocks.is_empty(),
            visible,
            "block display in visible empty section"
        );
        assert_eq!(
            !mob_verts.is_empty(),
            visible,
            "mob in visible empty section"
        );

        // Above and below build height: always visible even when uncompiled.
        for (id, y, key) in [
            (7003, 340.0, "above_build_height_uncompiled"),
            (7004, -80.0, "below_build_height_uncompiled"),
        ] {
            game.text_displays.clear();
            assert!(game.spawn_remote_object(
                id,
                "minecraft:text_display",
                [0.5, y, 3.5],
                (0.0, 0.0),
                0,
            ));
            game.text_displays.get_mut(&id).unwrap().metadata(&[(
                23,
                0,
                Value::Text(Tag::String("Out".into())),
            )]);
            game.tick_text_displays();
            normal.clear();
            game.text_display_geometry(DVec3::new(0.5, y, 0.5), 1.0, &mut normal, &mut see_through);
            assert_eq!(!normal.is_empty(), gate[key].as_bool().unwrap(), "{key}");
        }
    });
}
