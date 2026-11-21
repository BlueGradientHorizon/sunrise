use super::*;
use glam::Vec4;
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

const SECTION_WITNESS: &str =
    include_str!("../../../../tools/witness/26.3/section_visibility/witness.json");

fn parse_f64_bits(hex: &str) -> f64 {
    let stripped = hex
        .strip_prefix("0x")
        .or_else(|| hex.strip_prefix("0X"))
        .unwrap_or(hex);
    f64::from_bits(
        u64::from_str_radix(stripped, 16)
            .unwrap_or_else(|e| panic!("invalid f64 hex {hex:?}: {e}")),
    )
}

fn parse_f32_bits(hex: &str) -> f32 {
    let stripped = hex
        .strip_prefix("0x")
        .or_else(|| hex.strip_prefix("0X"))
        .unwrap_or(hex);
    f32::from_bits(
        u32::from_str_radix(stripped, 16)
            .unwrap_or_else(|e| panic!("invalid f32 hex {hex:?}: {e}")),
    )
}

fn parse_camera_bits(arr: &[Value]) -> DVec3 {
    DVec3::new(
        parse_f64_bits(arr[0].as_str().expect("camera_bits[0]")),
        parse_f64_bits(arr[1].as_str().expect("camera_bits[1]")),
        parse_f64_bits(arr[2].as_str().expect("camera_bits[2]")),
    )
}

fn parse_sections(arr: &[Value]) -> Vec<[i32; 3]> {
    let mut out: Vec<[i32; 3]> = arr
        .iter()
        .map(|s| {
            let a = s.as_array().expect("section triple");
            [
                a[0].as_i64().expect("sx") as i32,
                a[1].as_i64().expect("sy") as i32,
                a[2].as_i64().expect("sz") as i32,
            ]
        })
        .collect();
    out.sort_unstable();
    out
}

/// Vanilla 26.3 witness (`tools/witness/26.3/section_visibility/witness.json`): GPU culling vertical clamp with reach off.
#[test]
fn gpu_culling_tables_and_keeps_enforce_vanilla_26_3_vertical_clamp() {
    let witness: Value = serde_json::from_str(SECTION_WITNESS).expect("valid witness JSON");
    assert_eq!(witness["meta"]["game_version"].as_str(), Some("26.3"));
    let cases = witness["cases"].as_array().expect("cases array");
    let all_frustum = Frustum::from_planes([Vec4::W; 6]);
    let mut words = Vec::new();
    let mut checked = 0;

    for case in cases {
        if case.get("vis_overrides").is_some() {
            continue;
        }
        let name = case["name"].as_str().expect("name");
        let min_y = case["min_y"].as_i64().expect("min_y") as i32;
        let sections = case["sections"].as_i64().expect("sections") as i32;
        let min_sec_y = case["min_section_y"].as_i64().expect("min_section_y") as i32;
        let rd = case["render_distance"].as_i64().expect("render_distance") as i32;
        let camera = parse_camera_bits(case["camera_bits"].as_array().expect("camera_bits"));
        let cam_sec = case["camera_section"].as_array().expect("camera_section");
        let cam_cx = cam_sec[0].as_i64().expect("cam_cx") as i32;
        let cam_cz = cam_sec[2].as_i64().expect("cam_cz") as i32;
        let non_empty: Option<HashSet<i32>> = case
            .get("non_empty_world_sy")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|v| v.as_i64().unwrap() as i32).collect());
        let expected_all = parse_sections(
            case["octree_all_sections"]
                .as_array()
                .expect("octree_all_sections"),
        );

        let input = FrameInputs::new(camera, min_y, sections, rd, &all_frustum);
        let mut params =
            tables(&input, &mut words).unwrap_or_else(|| panic!("case {name}: tables"));
        params.pages = 1;

        let mut actual = Vec::new();
        for x in cam_cx - rd - 1..=cam_cx + rd + 1 {
            for z in cam_cz - rd - 1..=cam_cz + rd + 1 {
                for y in 0..sections {
                    let sy = y + min_sec_y;
                    if non_empty.as_ref().is_some_and(|set| !set.contains(&sy)) {
                        continue;
                    }
                    let record = Record::new((x, y, z), 0, 0, 1, UNBOUNDED);
                    if super::tests::keeps(&params, &words, &record) {
                        actual.push([x, sy, z]);
                    }
                }
            }
        }
        actual.sort_unstable();
        assert_eq!(
            actual, expected_all,
            "case {name}: GPU culling tables/keeps diverge from vanilla 26.3 vertical clamp"
        );

        if case.get("frustum_visible_count").and_then(Value::as_u64) == Some(0) {
            let f = case["frustum"].as_object().expect("frustum");
            let yaw = f["yaw"].as_f64().expect("yaw") as f32;
            let pitch = f["pitch"].as_f64().expect("pitch") as f32;
            let fov = f["fov"].as_f64().expect("fov") as f32;
            let aspect = f["aspect"].as_f64().expect("aspect") as f32;
            let far = ((rd * 16 * 4) as f32).max(12.0 * 16.0);
            let proj = super::super::world_projection(fov, aspect, far);
            let view = crate::camera::view_matrix(yaw, pitch);
            let frustum = Frustum::from_planes(super::super::frustum(proj * view));
            let input = FrameInputs::new(camera, min_y, sections, rd, &frustum);
            let mut params =
                tables(&input, &mut words).unwrap_or_else(|| panic!("case {name}: frustum tables"));
            params.pages = 1;
            let mut kept = 0usize;
            for x in cam_cx - rd - 1..=cam_cx + rd + 1 {
                for z in cam_cz - rd - 1..=cam_cz + rd + 1 {
                    for y in 0..sections {
                        let sy = y + min_sec_y;
                        if non_empty.as_ref().is_some_and(|set| !set.contains(&sy)) {
                            continue;
                        }
                        let record = Record::new((x, y, z), 0, 0, 1, rounded_box(None));
                        if super::tests::keeps(&params, &words, &record) {
                            kept += 1;
                        }
                    }
                }
            }
            assert_eq!(
                kept, 0,
                "case {name}: GPU culling kept {kept} sections where vanilla draws 0"
            );
        }
        checked += 1;
    }
    assert!(
        checked >= 15,
        "expected at least 15 open-world cases, got {checked}"
    );
}

/// Vanilla 26.3 genpvp menu witness (`tools/witness/26.3/genpvp_menu/settled.json`): GPU culling keeps zero lobby sections.
#[test]
fn gpu_culling_genpvp_menu_witness_culls_lobby_without_cave_culling() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/witness/26.3/genpvp_menu/settled.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing menu witness {}: {e}", path.display()));
    let witness: Value = serde_json::from_str(&text).expect("valid settled.json");
    assert_eq!(witness["meta"]["game_version"].as_str(), Some("26.3"));

    let camera = parse_camera_bits(
        witness["camera"]["camera_pos_bits"]
            .as_array()
            .expect("camera.camera_pos_bits"),
    );
    let fov = parse_f32_bits(
        witness["frame"]["fov_bits"]
            .as_str()
            .expect("frame.fov_bits"),
    );
    let visible_count = witness["frame"]["visible_terrain_sections"]
        .as_u64()
        .expect("frame.visible_terrain_sections") as usize;
    let visible_sections = parse_sections(
        witness["frame"]["visible_sections"]
            .as_array()
            .expect("frame.visible_sections"),
    );
    assert_eq!(visible_count, visible_sections.len());

    let client_rd = witness["meta"]["options"]["renderDistance"]
        .as_i64()
        .or_else(|| {
            witness["meta"]["options"]["renderDistance"]
                .as_str()
                .and_then(|s| s.parse::<i64>().ok())
        })
        .unwrap_or(2) as i32;
    let effective_rd = client_rd.min(2);

    let far = ((effective_rd * 16 * 4) as f32).max(12.0 * 16.0);
    let proj = super::super::world_projection(fov, 1280.0 / 800.0, far);
    let down = Frustum::from_planes(super::super::frustum(
        proj * crate::camera::view_matrix(0.0, 90.0),
    ));
    let mut words = Vec::new();
    let input = FrameInputs::new(camera, -64, 24, effective_rd, &down);
    let mut params = tables(&input, &mut words).expect("tables fit");
    params.pages = 1;

    let mut kept = Vec::new();
    for cx in -3..=3 {
        for cz in -3..=3 {
            for sy in [0, 1, 2, 3, 8, 9, 10, 11] {
                let record = Record::new((cx, sy + 4, cz), 0, 0, 1, rounded_box(None));
                if super::tests::keeps(&params, &words, &record) {
                    kept.push([cx, sy, cz]);
                }
            }
        }
    }
    kept.sort_unstable();
    assert_eq!(
        kept, visible_sections,
        "GPU culling must keep 0 genpvp lobby sections when looking down at rd={effective_rd}"
    );
}
