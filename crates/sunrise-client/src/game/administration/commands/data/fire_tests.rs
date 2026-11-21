use super::{DataRef, Target};
use crate::game::native_entity_refs::tests::with_game;
use sunrise_client::motion::PlayerMotion;
use sunrise_core::geometry::Vec3;
use sunrise_core::nbt::Tag;

const WITNESS_26_3: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/witness/26.3/entity_fire_ticks.tsv"
));

struct WitnessRow<'a> {
    kind: &'a str,
    initial_fire: &'a str,
    nbt: &'a str,
    ticks: usize,
    expected: Option<Tag>,
}

struct Active<'a> {
    kind: &'a str,
    nbt: &'a str,
    target: Target,
    elapsed: usize,
}

fn fire(game: &mut crate::game::Game<'_>, target: Target) -> Option<Tag> {
    game.data_of(&DataRef::Entity(target))
        .ok()
        .and_then(|tag| tag.get("Fire").cloned())
}

fn summon_target(game: &mut crate::game::Game<'_>, kind: &str, nbt: &str) -> Target {
    let returned = game.summon_with_nbt(kind, [8.5, 220.0, 8.5], nbt).unwrap();
    match kind {
        "minecraft:evoker_fangs" => Target::Fangs(game.fangs.list.last().unwrap().id),
        "minecraft:area_effect_cloud" => Target::Cloud(game.breath_clouds.last().unwrap().id),
        _ => returned.unwrap_or_else(|| panic!("missing summon target for {kind}")),
    }
}

fn tick_fire_entities(game: &mut crate::game::Game<'_>) {
    game.ticked_entities_through = u32::MAX;
    game.tick_summoned_eyes();
    game.tick_fangs();
    game.tick_ominous_item_spawners();
    game.tick_potion_clouds();
    game.tick_projectiles_now().unwrap();
}

fn parse_witness_26_3() -> Vec<WitnessRow<'static>> {
    let mut lines = WITNESS_26_3
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'));
    assert_eq!(
        lines.next(),
        Some("kind\tinitial_fire\tnbt\tticks\tstate\tfire")
    );
    let rows: Vec<WitnessRow<'static>> = lines
        .map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            assert_eq!(parts.len(), 6, "malformed 26.3 witness row: {line}");
            let expected = match (parts[4], parts[5]) {
                ("alive", raw) => Some(Tag::Short(raw.parse().unwrap())),
                ("gone", "gone") => None,
                other => panic!("unexpected 26.3 witness state: {other:?}"),
            };
            WitnessRow {
                kind: parts[0],
                initial_fire: parts[1],
                nbt: parts[2],
                ticks: parts[3].parse().unwrap(),
                expected,
            }
        })
        .collect();
    assert!(!rows.is_empty());
    rows
}

/// Vanilla 26.3 witness parity (#2683): compare Fire ticks against the recorded oracle output.
#[test]
#[ignore = "daily"]
fn fire_changes_on_eyes_fangs_spawners_clouds_and_bullets() {
    let rows = parse_witness_26_3();
    with_game(|game| {
        game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
        game.update_chunk_tickets();
        let mut active: Option<Active<'_>> = None;
        for row in rows {
            let same_case = matches!(
                &active,
                Some(a) if a.kind == row.kind && a.nbt == row.nbt && row.ticks >= a.elapsed
            );
            if !same_case {
                if let Some(prev) = active.take() {
                    game.kill_target(prev.target);
                }
                let target = summon_target(game, row.kind, row.nbt);
                active = Some(Active {
                    kind: row.kind,
                    nbt: row.nbt,
                    target,
                    elapsed: 0,
                });
            }
            let current = active.as_mut().unwrap();
            while current.elapsed < row.ticks {
                tick_fire_entities(game);
                current.elapsed += 1;
            }
            assert_eq!(
                fire(game, current.target),
                row.expected,
                "vanilla 26.3 mismatch for {} initial_fire={} ticks={}",
                row.kind,
                row.initial_fire,
                row.ticks
            );
        }
        if let Some(prev) = active.take() {
            game.kill_target(prev.target);
        }
    });
}
