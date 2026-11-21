use crate::game::native_entity_refs::tests::{
    bind_local_player, prepare_entity_ticking_neighborhood, with_game,
};
use serde_json::Value;
use sunrise_client::motion::PlayerMotion;
use sunrise_core::geometry::Vec3;
use sunrise_core::nbt::Tag;
use sunrise_core::position::ChunkPos;

const WITNESS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/witness/26.3/cloud_water.json"
));

fn offset_coord(base: i32, offset: &str) -> String {
    match offset.split_once('.') {
        Some((whole, frac)) => format!("{}.{frac}", base + whole.parse::<i32>().unwrap()),
        None => (base + offset.parse::<i32>().unwrap()).to_string(),
    }
}

fn parse_bits(case: &Value, key: &str) -> Tag {
    let hex = case[key].as_str().unwrap();
    Tag::Double(u64::from_str_radix(hex, 16).unwrap())
}

/// #2359 (vanilla 26.3): area_effect_cloud fall_distance matches the recorded 26.3 witness.
#[test]
#[ignore = "daily"]
fn a_cloud_in_water_resets_its_fall_distance_and_a_dry_one_keeps_it() {
    let witness: Value = serde_json::from_str(WITNESS).unwrap();
    assert_eq!(witness["minecraft"].as_str(), Some("26.3"));
    let water = witness["water_offset"].as_array().unwrap();
    let cases = witness["cases"].as_array().unwrap();
    assert!(!cases.is_empty());

    with_game(|game| {
        prepare_entity_ticking_neighborhood(game, ChunkPos::new(0, 0));
        prepare_entity_ticking_neighborhood(game, ChunkPos::new(1, 0));
        game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
        game.update_chunk_tickets();
        bind_local_player(game, 1, 99);
        game.set_allow_commands(true);
        game.build_registries();
        assert!(game.run_command("forceload add 0 0 31 15"));
        let origin = [0_i32, 64, 0];
        let wx = origin[0] + water[0].as_i64().unwrap() as i32;
        let wy = origin[1] + water[1].as_i64().unwrap() as i32;
        let wz = origin[2] + water[2].as_i64().unwrap() as i32;
        assert!(game.run_command(&format!("setblock {wx} {wy} {wz} minecraft:water")));

        let mut spawned = Vec::with_capacity(cases.len());
        for (index, case) in cases.iter().enumerate() {
            let n = (index + 1) as i32;
            let uuid = [2359, 0, n, 1];
            let offset = case["offset"].as_array().unwrap();
            let x = offset_coord(origin[0], offset[0].as_str().unwrap());
            let y = offset_coord(origin[1], offset[1].as_str().unwrap());
            let z = offset_coord(origin[2], offset[2].as_str().unwrap());
            let initial = case["initial_fall_distance"].as_str().unwrap();
            let duration = case["duration"].as_i64().unwrap();
            let radius = case["radius"]
                .as_str()
                .map_or_else(String::new, |r| format!(",Radius:{r}f"));
            let command = format!(
                "summon minecraft:area_effect_cloud {x} {y} {z} {{UUID:[I;2359,0,{n},1],fall_distance:{initial}d,Duration:{duration}{radius}}}"
            );
            assert!(game.run_command(&command), "{command}");
            let id = game.breath_clouds.last().unwrap().id;
            spawned.push((id, uuid, case));
        }

        let check = |game: &mut crate::game::Game<'_>, key: &str| {
            for &(id, uuid, case) in &spawned {
                let name = case["name"].as_str().unwrap();
                let expected = Some(parse_bits(case, key));
                let saved = {
                    let cloud = game.breath_clouds.iter().find(|c| c.id == id).unwrap();
                    super::potion_cloud_to_nbt(cloud, uuid, game.persist_raw(id))
                        .get("fall_distance")
                        .cloned()
                };
                assert_eq!(saved, expected, "{name} ({key}, chunk save)");
                let target = game.target_with_uuid(uuid).unwrap();
                let live = game
                    .command_target_nbt(target)
                    .unwrap()
                    .get("fall_distance")
                    .cloned();
                assert_eq!(live, expected, "{name} ({key}, /data get)");
            }
        };

        check(game, "tick0_fall_bits");
        game.tick_potion_clouds();
        check(game, "tick1_fall_bits");
    });
}
