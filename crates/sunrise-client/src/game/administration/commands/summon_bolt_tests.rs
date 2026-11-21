use crate::game::command_harness::spans;
use crate::game::native_entity_refs::tests::{
    bind_local_player, prepare_entity_ticking_neighborhood, with_game,
};
use serde_json::{Value, json};
use sunrise_client::motion::PlayerMotion;
use sunrise_core::geometry::Vec3;
use sunrise_core::position::ChunkPos;

const WITNESS_26_3: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/witness/26.3/summon_bolt_nbt.json"
));

/// Replays the vanilla 26.3 witness for `/summon minecraft:lightning_bolt` with trailing NBT (#2690).
#[test]
#[ignore = "daily"]
fn a_bolt_summoned_with_nbt_strikes_and_goes() {
    let witness: Value = serde_json::from_str(WITNESS_26_3).unwrap();
    assert_eq!(witness["minecraft"], "26.3");
    let steps = witness["steps"].as_array().unwrap();
    assert!(!steps.is_empty());

    let _language =
        sunrise_client::lang::isolate_publication(sunrise_client::lang::Language::of("en_us", []));
    with_game(|game| {
        prepare_entity_ticking_neighborhood(game, ChunkPos::new(0, 0));
        game.player = Some(PlayerMotion::new(game.motion, Vec3::new(8.5, 230.0, 8.5)).unwrap());
        bind_local_player(game, 1, 99);
        game.creative = true;
        game.ui.difficulty = 2;
        game.set_allow_commands(true);
        game.build_registries();
        assert!(game.run_command("forceload add 0 0"));
        for setup_cmd in [
            "gamerule spawn_mobs false",
            "gamerule mob_drops false",
            "gamerule entity_drops false",
            "gamerule advance_time false",
            "gamerule advance_weather false",
        ] {
            let _ = game.run_command(setup_cmd);
        }

        let others_before = game.others.len();
        for (index, step) in steps.iter().enumerate() {
            match step["op"].as_str().unwrap() {
                "cmd" => {
                    let cmd = step["cmd"].as_str().unwrap();
                    let line = cmd.strip_prefix('/').unwrap_or(cmd);
                    game.chat_capture = Some(Vec::new());
                    game.cmds.results = Some(Vec::new());
                    game.as_console(|g| g.run_command(line));
                    let messages = game.chat_capture.take().unwrap_or_default();
                    let results = game.cmds.results.take().unwrap_or_default();
                    if step["record"].as_bool() != Some(false) {
                        let actual_messages = Value::Array(messages.iter().map(spans).collect());
                        let actual_results =
                            Value::Array(results.iter().map(|(s, v)| json!([s, v])).collect());
                        assert_eq!(
                            actual_messages, step["messages"],
                            "step {index} ({cmd}): messages differ from 26.3 witness"
                        );
                        assert_eq!(
                            actual_results, step["results"],
                            "step {index} ({cmd}): results differ from 26.3 witness"
                        );
                        if cmd.starts_with("/summon minecraft:lightning_bolt ") {
                            let summoned = step["results"][0][0].as_bool().unwrap();
                            assert_eq!(
                                game.bolts.len(),
                                usize::from(summoned),
                                "step {index} ({cmd}): bolt count differs from 26.3 witness"
                            );
                            assert_eq!(
                                game.others.len(),
                                others_before,
                                "step {index} ({cmd}): lightning_bolt must not be stored in others"
                            );
                        }
                        if cmd == "/execute if entity @e[type=minecraft:lightning_bolt]"
                            && step["results"][0][0].as_bool() == Some(false)
                        {
                            assert!(
                                game.bolts.is_empty(),
                                "step {index} ({cmd}): bolt still alive when 26.3 witness saw none"
                            );
                        }
                    }
                }
                "tick" => {
                    let n = step["n"].as_u64().unwrap();
                    for _ in 0..n {
                        game.tick_server(false).unwrap();
                    }
                }
                other => panic!("step {index}: unexpected witness op {other}"),
            }
        }
        assert!(
            game.bolts.is_empty(),
            "bolts must be gone after 26.3 witness"
        );
        assert_eq!(
            game.others.len(),
            others_before,
            "no stored lightning_bolt entity"
        );
    });
}
