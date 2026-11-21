use super::*;
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

/// Vanilla 26.3 SectionOcclusionGraph witness (`tools/witness/26.3/section_visibility/witness.json`).
#[test]
fn occlusion_reaches_exact_vanilla_26_3_sections_across_heights_and_radii() {
    let witness: Value = serde_json::from_str(SECTION_WITNESS).expect("valid witness JSON");
    assert_eq!(witness["meta"]["game_version"].as_str(), Some("26.3"));
    let cases = witness["cases"].as_array().expect("cases array");
    let mut checked = 0;

    for case in cases {
        if !case["smart_cull"].as_bool().expect("smart_cull") {
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
        let expected = parse_sections(
            case["reached_sections"]
                .as_array()
                .expect("reached_sections"),
        );

        let mut o = Occlusion::new(min_y, sections);
        if let Some(overrides) = case.get("vis_overrides").and_then(Value::as_array) {
            for ov in overrides {
                let s = ov["section"].as_array().expect("override section");
                let sx = s[0].as_i64().expect("sx") as i32;
                let sy = s[1].as_i64().expect("sy") as i32;
                let sz = s[2].as_i64().expect("sz") as i32;
                let mask = ov["mask"].as_u64().expect("mask");
                o.set((sx, sy - min_sec_y, sz), mask);
            }
        }
        o.settle(camera, rd);
        let reach = o.reach().expect("settled occlusion exports Reach");

        let mut actual = Vec::new();
        for x in cam_cx - rd - 1..=cam_cx + rd + 1 {
            for z in cam_cz - rd - 1..=cam_cz + rd + 1 {
                for y in -1..=sections {
                    let key = (x, y, z);
                    let got = o.contains(key);
                    assert_eq!(
                        reach.contains(key),
                        got,
                        "case {name}: Reach::contains disagrees with Occlusion::contains at {key:?}"
                    );
                    if got {
                        actual.push([x, y + min_sec_y, z]);
                    }
                }
            }
        }
        actual.sort_unstable();
        assert_eq!(
            actual, expected,
            "case {name}: reached sections diverge from vanilla 26.3 SectionOcclusionGraph"
        );
        checked += 1;
    }
    assert!(
        checked >= 18,
        "expected at least 18 smart_cull cases, got {checked}"
    );
}

/// Fail open (`None`) for entity occlusion outside vanilla's vertical clamp (`witness.json`, 26.3).
#[test]
fn occlusion_visible_for_camera_is_unknown_outside_vanilla_vertical_band() {
    let witness: Value = serde_json::from_str(SECTION_WITNESS).expect("valid witness JSON");
    let cases = witness["cases"].as_array().expect("cases array");
    let find = |target: &str| {
        cases
            .iter()
            .find(|c| c["name"].as_str() == Some(target))
            .unwrap_or_else(|| panic!("missing case {target}"))
    };

    for name in [
        "genpvp_menu_level",
        "cave_floor_with_shaft_and_vertical_clamp",
    ] {
        let case = find(name);
        let min_y = case["min_y"].as_i64().expect("min_y") as i32;
        let sections = case["sections"].as_i64().expect("sections") as i32;
        let min_sec_y = case["min_section_y"].as_i64().expect("min_section_y") as i32;
        let rd = case["render_distance"].as_i64().expect("render_distance") as i32;
        let camera = parse_camera_bits(case["camera_bits"].as_array().expect("camera_bits"));
        let reached: HashSet<[i32; 3]> = parse_sections(
            case["reached_sections"]
                .as_array()
                .expect("reached_sections"),
        )
        .into_iter()
        .collect();

        let mut o = Occlusion::new(min_y, sections);
        if let Some(overrides) = case.get("vis_overrides").and_then(Value::as_array) {
            for ov in overrides {
                let s = ov["section"].as_array().expect("override section");
                o.set(
                    (
                        s[0].as_i64().unwrap() as i32,
                        s[1].as_i64().unwrap() as i32 - min_sec_y,
                        s[2].as_i64().unwrap() as i32,
                    ),
                    ov["mask"].as_u64().unwrap(),
                );
            }
        }
        o.settle(camera, rd);
        let cam_sec = o.columns.section_of(camera);

        // One section below vanilla's vertical clamp must be unknown (None), never Some(true/false).
        let below_y = cam_sec.1 - rd - 1;
        assert!(below_y >= 0, "case {name}: expected below_y >= 0");
        assert_eq!(
            o.visible_for_camera((cam_sec.0, below_y, cam_sec.2), cam_sec),
            None,
            "case {name}: section below vertical clamp must return None"
        );

        // Inside the vertical band at the camera column, visible_for_camera must match vanilla.
        for dy in -rd..=rd {
            let y = cam_sec.1 + dy;
            if !(0..sections).contains(&y) {
                continue;
            }
            let expected = reached.contains(&[cam_sec.0, y + min_sec_y, cam_sec.2]);
            assert_eq!(
                o.visible_for_camera((cam_sec.0, y, cam_sec.2), cam_sec),
                Some(expected),
                "case {name}: visible_for_camera mismatch at y={y}"
            );
        }
    }
}

/// Vanilla 26.3 genpvp menu witness (`tools/witness/26.3/genpvp_menu/settled.json`): zero lobby sections reached.
#[test]
fn occlusion_genpvp_menu_witness_reaches_zero_lobby_sections() {
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
    let visible_count = witness["frame"]["visible_terrain_sections"]
        .as_u64()
        .expect("frame.visible_terrain_sections");
    let visible_sections = parse_sections(
        witness["frame"]["visible_sections"]
            .as_array()
            .expect("frame.visible_sections"),
    );
    assert_eq!(visible_count as usize, visible_sections.len());

    let client_rd = witness["meta"]["options"]["renderDistance"]
        .as_i64()
        .or_else(|| {
            witness["meta"]["options"]["renderDistance"]
                .as_str()
                .and_then(|s| s.parse::<i64>().ok())
        })
        .unwrap_or(2) as i32;
    let effective_rd = client_rd.min(2);

    let mut o = Occlusion::new(-64, 24);
    o.settle(camera, effective_rd);

    let mut reached_lobby = Vec::new();
    for cx in -3..=3 {
        for cz in -3..=3 {
            for sy in [0, 1, 2, 3, 8, 9, 10, 11] {
                if o.contains((cx, sy + 4, cz)) {
                    reached_lobby.push([cx, sy, cz]);
                }
            }
        }
    }
    reached_lobby.sort_unstable();
    assert_eq!(
        reached_lobby, visible_sections,
        "genpvp menu camera at {camera} (rd={effective_rd}) must not reach lobby sections below vertical clamp"
    );
}
