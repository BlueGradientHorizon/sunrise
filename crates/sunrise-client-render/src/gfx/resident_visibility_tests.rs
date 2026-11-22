use super::*;
use glam::Vec4;
use serde_json::Value;
use std::collections::HashSet;

fn section_witness() -> Value {
    let path = std::env::var("SUNRISE_SECTION_VISIBILITY_WITNESS").expect(
        "SUNRISE_SECTION_VISIBILITY_WITNESS must be set (run via tools/test_visibility_parity.py)",
    );
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read section visibility witness {path}: {e}"));
    serde_json::from_str(&text).expect("valid witness JSON")
}

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

/// Vanilla 26.3 witness (SUNRISE_SECTION_VISIBILITY_WITNESS): vertical clamp with cave culling off.
#[test]
#[ignore = "run by tools/test_visibility_parity.py"]
fn resident_walk_enforces_vanilla_26_3_vertical_clamp_without_cave_culling() {
    let witness = section_witness();
    assert_eq!(witness["meta"]["game_version"].as_str(), Some("26.3"));
    let cases = witness["cases"].as_array().expect("cases array");
    let all_frustum = Frustum::from_planes([Vec4::W; 6]);
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

        let mut r = Resident::default();
        r.set_min_y(min_y);
        for x in cam_cx - rd - 1..=cam_cx + rd + 1 {
            for z in cam_cz - rd - 1..=cam_cz + rd + 1 {
                for y in 0..sections {
                    let sy = y + min_sec_y;
                    if non_empty.as_ref().is_none_or(|set| set.contains(&sy)) {
                        r.insert((x, y, z), ());
                    }
                }
            }
        }

        let mut out = Vec::new();
        r.visible(camera, rd, &all_frustum, |_| true, |_, _| false, &mut out);
        let mut actual: Vec<[i32; 3]> = out
            .iter()
            .map(|v| [v.key.0, v.key.1 + min_sec_y, v.key.2])
            .collect();
        actual.sort_unstable();
        assert_eq!(
            actual, expected_all,
            "case {name}: Resident::visible diverges from vanilla 26.3 vertical render-distance clamp"
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
            out.clear();
            r.visible(camera, rd, &frustum, |_| true, |_, _| false, &mut out);
            assert!(
                out.is_empty(),
                "case {name}: Resident::visible returned {} sections where vanilla draws 0",
                out.len()
            );
        }
        checked += 1;
    }
    assert!(
        checked >= 15,
        "expected at least 15 open-world cases, got {checked}"
    );
}
