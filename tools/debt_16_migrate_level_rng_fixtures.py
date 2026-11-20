#!/usr/bin/env python3
"""Migrate the four level RNG fixtures before the raw state API is removed."""

from pathlib import Path


GAME = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src/game"
REPLACEMENTS = {
    "blocks/world_fluids_tests.rs": (
        (
            """    use mod_api::{
        BlockPos, BlockUpdateFlags, LevelHost, LevelRandomHost, LevelTicksHost, LevelWriteHost,
        RandomSource,
    };
""",
            "    use mod_api::{BlockPos, BlockUpdateFlags, LevelWriteHost};\n",
        ),
        (
            """                let mut host = JournalLevelWrite::new(game, level).unwrap();
                assert!(host.block_entity(AT).is_some());
""",
            """                let mut host = JournalLevelWrite::new(game, level).unwrap();
                let host: &mut dyn LevelWriteHost = &mut host;
                assert!(host.block_entity(AT).is_some());
""",
        ),
        (
            """                let mut random = host.random().unwrap();
                random.next_int();
                assert!(host.set_random(random));
""",
            "                host.with_random(|random| random.next_int()).unwrap();\n",
        ),
    ),
    "native_hooks/shared/item_fall_on_tests.rs": (
        ("use mod_api::RandomSource as _;\n", ""),
        (
            """        let mut random = host.random().expect("authoritative landing random");
        assert_eq!(random.next_int(), 1553932502);
        assert!(host.set_random(random));
""",
            """        let draw = host
            .with_random(|random| random.next_int())
            .expect("authoritative landing random");
        assert_eq!(draw, 1553932502);
""",
        ),
    ),
    "modding/native_level_refs.rs": (
        (
            '            assert!(parked.random(&*host).is_none(), "not open yet");\n',
            """            assert!(
                parked
                    .with_random::<()>(host, |_| panic!("unopened level lent its random stream"))
                    .is_none(),
                "not open yet"
            );
""",
        ),
    ),
    "modding/native_place_feature_tests.rs": (
        (
            "    use mod_api::{LegacyRandomSource, RandomSource};\n",
            "    use sunrise_core::random::{LegacyRandom, RandomSource};\n",
        ),
        (
            "            assert!(level.set_random(game, LegacyRandomSource::new(seed)));\n",
            "            *game.world.random = LegacyRandom::new(seed);\n",
        ),
        (
            """            let mut expected = LegacyRandomSource::new(seed);
            let rarity = expected.next_float();
            assert_eq!(rarity < 1.0 / 200.0, reached);
            if reached {
                expected.next_int_bound(16);
                expected.next_int_bound(16);
            }
            assert_eq!(
                <(u64, Option<f64>)>::from(&level.random(game).unwrap()),
""",
            """            let mut expected = LegacyRandom::new(seed);
            let rarity = expected.next_float();
            assert_eq!(rarity < 1.0 / 200.0, reached);
            if reached {
                expected.next_int_bound(16).unwrap();
                expected.next_int_bound(16).unwrap();
            }
            assert_eq!(
                <(u64, Option<f64>)>::from(&*game.world.random),
""",
        ),
    ),
}

# Check every replacement before writing any file.
updated = {}
for name, replacements in REPLACEMENTS.items():
    path = GAME / name
    source = path.read_text(encoding="utf-8")
    for old, new in replacements:
        count = source.count(old)
        if count != 1:
            raise SystemExit(f"{name}: expected one match, found {count}: {old.splitlines()[0]}")
        source = source.replace(old, new, 1)
    updated[path] = source

for path, source in updated.items():
    path.write_text(source, encoding="utf-8")
