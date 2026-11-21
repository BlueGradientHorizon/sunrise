use crate::game::native_entity_refs::tests::with_game;
use serde::Deserialize;
use sunrise_client::items::ItemStack;
use sunrise_client::motion::PlayerMotion;
use sunrise_core::geometry::Vec3;

const WITNESS_26_3: &str =
    include_str!("../../../../../../tools/witness/26.3/fangs_ominous_spawner_ticking_gate.json");

#[derive(Deserialize)]
struct TickingGateWitness {
    game_version: String,
    player_pos: [f64; 3],
    view_distance: i32,
    simulation_distance: i32,
    cases: Vec<TickingGateCase>,
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum TickingGateCase {
    #[serde(rename = "evoker_fangs")]
    EvokerFangs {
        name: String,
        pos: [f64; 3],
        forced_chunks: Vec<[i32; 2]>,
        initial_warmup: i32,
        ticks: u32,
        expected_present: bool,
        expected_warmup: Option<i32>,
    },
    #[serde(rename = "ominous_item_spawner")]
    OminousItemSpawner {
        name: String,
        pos: [f64; 3],
        forced_chunks: Vec<[i32; 2]>,
        item: String,
        spawn_item_after_ticks: i32,
        ticks: u32,
        expected_present: bool,
        expected_spawned_items: usize,
    },
}

/// #2361 (vanilla 26.3): evoker fangs and ominous item spawners match the recorded ticking-chunk gate witness.
#[test]
#[ignore = "daily"]
fn fangs_and_an_ominous_item_spawner_outside_the_ticking_chunks_do_not_tick() {
    let witness: TickingGateWitness =
        serde_json::from_str(WITNESS_26_3).expect("parse 26.3 ticking-gate witness");
    assert_eq!(witness.game_version, "26.3");
    assert!(!witness.cases.is_empty());

    with_game(|game| {
        game.settings.render_distance = witness.view_distance;
        game.settings.simulation_distance = witness.simulation_distance;
        let [px, py, pz] = witness.player_pos;
        game.player = Some(PlayerMotion::new(game.motion, Vec3::new(px, py, pz)).unwrap());

        for case in &witness.cases {
            game.fangs.list.clear();
            game.ominous_spawners.clear();
            game.dropped.clear();
            game.streaming.forced.clear();

            match case {
                TickingGateCase::EvokerFangs {
                    name,
                    pos,
                    forced_chunks,
                    initial_warmup,
                    ticks,
                    expected_present,
                    expected_warmup,
                } => {
                    for &[cx, cz] in forced_chunks {
                        game.streaming.forced.insert((cx, cz));
                    }
                    game.update_chunk_tickets();
                    game.add_fangs(*pos, 0.0, *initial_warmup, 0);
                    for _ in 0..*ticks {
                        game.tick_fangs();
                    }
                    let fangs = game.fangs.list.first();
                    assert_eq!(
                        game.fangs.list.len(),
                        usize::from(*expected_present),
                        "case {name}: fangs count"
                    );
                    assert_eq!(
                        fangs.map(|f| f.warmup),
                        *expected_warmup,
                        "case {name}: fangs warmup"
                    );
                    if let Some(fangs) = fangs {
                        assert_eq!(
                            fangs.pos.map(f64::to_bits),
                            pos.map(f64::to_bits),
                            "case {name}: fangs position bits"
                        );
                    }
                }
                TickingGateCase::OminousItemSpawner {
                    name,
                    pos,
                    forced_chunks,
                    item,
                    spawn_item_after_ticks,
                    ticks,
                    expected_present,
                    expected_spawned_items,
                } => {
                    for &[cx, cz] in forced_chunks {
                        game.streaming.forced.insert((cx, cz));
                    }
                    game.update_chunk_tickets();
                    game.add_ominous_item_spawner(
                        ItemStack::new(item, 1),
                        *pos,
                        *spawn_item_after_ticks,
                    );
                    for _ in 0..*ticks {
                        game.tick_ominous_item_spawners();
                    }
                    assert_eq!(
                        game.ominous_spawners.len(),
                        usize::from(*expected_present),
                        "case {name}: spawner count"
                    );
                    if let Some(spawner) = game.ominous_spawners.first() {
                        assert_eq!(
                            spawner.position.map(f64::to_bits),
                            pos.map(f64::to_bits),
                            "case {name}: spawner position bits"
                        );
                    }
                    assert_eq!(
                        game.dropped.len(),
                        *expected_spawned_items,
                        "case {name}: spawned items"
                    );
                    for dropped in game.dropped.iter() {
                        assert_eq!(
                            dropped.stack.item.as_ref(),
                            item.as_str(),
                            "case {name}: spawned item id"
                        );
                        assert_eq!(dropped.stack.count, 1, "case {name}: spawned item count");
                    }
                }
            }
        }
    });
}
