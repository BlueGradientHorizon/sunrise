use crate::game::native_entity_refs::tests::{prepare_entity_ticking_neighborhood, with_game};
use std::collections::BTreeMap;
use sunrise_client::motion::PlayerMotion;
use sunrise_core::geometry::Vec3;
use sunrise_core::nbt::Tag;
use sunrise_core::position::ChunkPos;

const WITNESS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/fixtures/26.3/fangs_fluid_current.tsv"
));

fn load_witness() -> BTreeMap<(&'static str, usize), [u64; 3]> {
    let mut rows = BTreeMap::new();
    for line in WITNESS.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.split('\t');
        let case = cols.next().expect("case column");
        let tick: usize = cols.next().expect("tick column").parse().expect("tick");
        let parse_bits = |s: &str| u64::from_str_radix(s, 16).expect("hex f64 bits");
        let bx = parse_bits(cols.next().expect("mx_bits"));
        let by = parse_bits(cols.next().expect("my_bits"));
        let bz = parse_bits(cols.next().expect("mz_bits"));
        assert_eq!(cols.count(), 3, "expected 8 columns in {line}");
        assert!(
            rows.insert((case, tick), [bx, by, bz]).is_none(),
            "duplicate witness row ({case}, {tick})"
        );
    }
    rows
}

fn motion_bits(v: Vec3) -> [u64; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

/// #2362 (vanilla 26.3): EvokerFangs fluid-current live and saved Motion match the recorded 26.3 witness bit-for-bit.
#[test]
#[ignore = "daily"]
fn fangs_in_a_current_are_pushed_and_fangs_in_still_water_are_not() {
    assert!(
        WITNESS.starts_with("# minecraft 26.3 "),
        "witness must be tagged with vanilla 26.3"
    );
    let expected = load_witness();
    with_game(|game| {
        prepare_entity_ticking_neighborhood(game, ChunkPos::new(0, 0));
        game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
        game.update_chunk_tickets();
        crate::game::native_entity_refs::tests::bind_local_player(game, 1, 99);
        game.set_allow_commands(true);
        game.build_registries();
        assert!(game.run_command("forceload add 0 0 15 15"));
        for command in [
            "fill 0 199 0 15 199 15 minecraft:stone",
            "fill 0 200 0 15 202 15 minecraft:barrier",
            "fill 0 200 0 15 202 15 minecraft:air",
            // Lava flowing north (-z) from its source through the fangs' cell.
            "setblock 2 200 4 minecraft:lava",
            "setblock 2 200 3 minecraft:lava[level=2]",
            // Water flowing north (-z) from its source through the fangs' cell.
            "setblock 6 200 4 minecraft:water",
            "setblock 6 200 3 minecraft:water[level=1]",
            // Diagonal water current (+x, -z) from a south source and an east level-3 flow.
            "setblock 10 200 4 minecraft:water",
            "setblock 11 200 3 minecraft:water[level=3]",
            "setblock 10 200 3 minecraft:water[level=1]",
            // Still water pools walled in on every side.
            "fill 1 200 9 3 200 11 minecraft:stone",
            "setblock 2 200 10 minecraft:water",
            "fill 5 200 9 7 200 11 minecraft:stone",
            "setblock 6 200 10 minecraft:water",
        ] {
            assert!(game.run_command(command), "{command}");
        }
        let cases = [
            (
                "lava_north",
                [2362, 0, 0, 1],
                "summon minecraft:evoker_fangs 2.5 200 3.5 {UUID:[I;2362,0,0,1],Warmup:100}",
            ),
            (
                "water_north",
                [2362, 0, 0, 2],
                "summon minecraft:evoker_fangs 6.5 200 3.5 {UUID:[I;2362,0,0,2],Warmup:100}",
            ),
            (
                "water_diagonal",
                [2362, 0, 0, 3],
                "summon minecraft:evoker_fangs 10.5 200 3.5 {UUID:[I;2362,0,0,3],Warmup:100}",
            ),
            (
                "still_water",
                [2362, 0, 0, 4],
                "summon minecraft:evoker_fangs 2.5 200 10.5 {UUID:[I;2362,0,0,4],Warmup:100}",
            ),
            (
                "still_water_loaded",
                [2362, 0, 0, 5],
                "summon minecraft:evoker_fangs 6.5 200 10.5 {UUID:[I;2362,0,0,5],Warmup:100,Motion:[0.125d,-0.25d,0.5d]}",
            ),
            (
                "dry_air_loaded",
                [2362, 0, 0, 6],
                "summon minecraft:evoker_fangs 10.5 200 10.5 {UUID:[I;2362,0,0,6],Warmup:100,Motion:[-0.0625d,0.03125d,0.25d]}",
            ),
        ];
        let mut spawned = Vec::with_capacity(cases.len());
        for (name, uuid, command) in cases {
            assert!(game.run_command(command), "{command}");
            let id = game.fangs.list.last().expect("summoned fangs").id;
            spawned.push((name, uuid, id));
        }
        let mut checked = 0usize;
        for tick in 0..=2usize {
            if tick > 0 {
                game.tick_fangs();
            }
            for &(name, uuid, id) in &spawned {
                let want = *expected
                    .get(&(name, tick))
                    .unwrap_or_else(|| panic!("missing witness row ({name}, {tick})"));
                let fangs = game
                    .fangs
                    .list
                    .iter()
                    .find(|f| f.id == id)
                    .unwrap_or_else(|| panic!("missing fangs {name}"));
                assert_eq!(
                    motion_bits(fangs.motion),
                    want,
                    "live motion bits mismatch for {name} at tick {tick}: {:?}",
                    fangs.motion
                );
                let saved = game.fangs_to_nbt(fangs, uuid);
                assert_eq!(
                    saved.get("Motion"),
                    Some(&Tag::List(want.into_iter().map(Tag::Double).collect())),
                    "saved Motion NBT bits mismatch for {name} at tick {tick}"
                );
                assert_eq!(
                    motion_bits(sunrise_client::entity_nbt::motion(&saved)),
                    want,
                    "re-read motion bits mismatch for {name} at tick {tick}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, expected.len(), "every witness row must be checked");
    });
}
