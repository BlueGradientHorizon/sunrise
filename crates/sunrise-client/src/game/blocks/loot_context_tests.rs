//! What a loot roll reads of its block entity and its place (`MatchBlock`, `LocationCheck`),
//! through the game's own rolls: a block's drops (`Block.getDrops`) and a table rolled at a
//! position (`LevelRef::roll_loot`, `/loot … fish`).
use super::*;
use std::sync::Arc;
use sunrise_client::chunk::Chunk;
use sunrise_core::{nbt::Tag, position::ChunkPos};

/// Runs `test` in a game whose world pack adds `tables` (`namespace/path` and JSON), with
/// the chunk at the origin loaded and a stone floor at y 99.
fn with_tables(name: &str, tables: &[(&str, &str)], test: impl FnOnce(&mut Game<'_>)) {
    crate::game::native_entity_refs::tests::with_game(|game| {
        let root = std::env::temp_dir().join(format!("loot-context-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        game.options.game_dir = Some(root.join("game"));
        game.build_registries();
        game.set_allow_commands(true);
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        game.world_root = root.join("world");
        let pack = game.world_root.join("datapacks/context");
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(
            pack.join("pack.mcmeta"),
            r#"{"pack":{"description":"loot context fixture"}}"#,
        )
        .unwrap();
        for (id, json) in tables {
            let (namespace, path) = id.split_once('/').unwrap();
            let file = pack.join(format!("data/{namespace}/loot_table/{path}.json"));
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, json).unwrap();
        }
        std::fs::write(
            game.world_root.join("datapacks.json"),
            r#"{"enabled":["vanilla","file/context"],"disabled":[]}"#,
        )
        .unwrap();
        let stack = game.reload_datapacks(false, &[]).unwrap();
        game.world_tables = game
            .stage_world_tables(&stack, false, &game.options.resources.clone())
            .unwrap()
            .tables;
        game.ensure_datapacks();
        assert!(game.run_command("forceload add 0 0 15 15"));
        assert!(game.run_command("fill 0 99 0 15 99 15 stone"));
        test(game);
        let _ = std::fs::remove_dir_all(&root);
    });
}

fn names(stacks: &[ItemStack]) -> Vec<&str> {
    let mut names: Vec<_> = stacks.iter().map(|s| &*s.item).collect();
    names.sort_unstable();
    names
}

/// `MatchBlock` tests the block state, then the predicate's NBT against the block entity's
/// `saveWithFullMetadata` (its id and position, its stored components, no `keepPacked`) and
/// its components against `collectComponents`.
#[test]
#[ignore = "daily"]
fn match_block_reads_the_block_entitys_full_data() {
    const CHEST: &str = r#"{"type":"minecraft:block","pools":[
        {"rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:diamond"}],
         "condition":{"type":"minecraft:match_block","blocks":"minecraft:chest",
            "predicates":{"minecraft:custom_data":"{k:1b}"}}},
        {"rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:gold_ingot"}],
         "condition":{"type":"minecraft:match_block",
            "nbt":"{id:\"minecraft:chest\",x:4,y:100,z:4,components:{\"minecraft:custom_data\":{k:1b}}}"}},
        {"rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:emerald"}],
         "condition":{"type":"minecraft:match_block","nbt":"{keepPacked:0b}"}},
        {"rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:apple"}],
         "condition":{"type":"minecraft:match_block","blocks":"minecraft:stone"}}
    ]}"#;
    with_tables("match", &[("minecraft/blocks/chest", CHEST)], |game| {
        assert!(game.run_command("setblock 4 100 4 minecraft:chest"));
        assert!(game.run_command("setblock 6 100 4 minecraft:chest"));
        // The chest at 4 was loaded from a save that kept a stored component patch, as a
        // chest placed from an item with custom data keeps it (`BlockEntity.components`).
        let stored = Tag::compound([
            ("id", Tag::String("minecraft:chest".into())),
            ("x", Tag::Int(4)),
            ("y", Tag::Int(100)),
            ("z", Tag::Int(4)),
            ("Items", Tag::List(Vec::new())),
            (
                "components",
                Tag::compound([(
                    "minecraft:custom_data",
                    Tag::compound([("k", Tag::Byte(1))]),
                )]),
            ),
        ]);
        game.world.with_chunk_extras((4, 100, 4), |extras| {
            extras.block_entity_sources.push(((4, 100, 4), stored))
        });
        let drops = |game: &mut Game<'_>, x: i32| {
            let at = BlockPos::new(x, 100, 4);
            let state = game.world.block(x, 100, 4);
            game.with_level(|_, level, _| level.block_drops_of(at, state, None, None))
        };
        assert_eq!(
            names(&drops(game, 4)),
            ["minecraft:diamond", "minecraft:gold_ingot"]
        );
        assert!(drops(game, 6).is_empty());
    });
}

/// `LocationCheck` needs an origin, holds with no predicate, and reads the level at the
/// origin moved by its offsets: a block there, and a dimension even where nothing is loaded.
#[test]
#[ignore = "daily"]
fn location_check_reads_the_level_at_the_roll_origin() {
    const WHERE: &str = r#"{"pools":[
        {"rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:diamond"}],
         "condition":{"type":"minecraft:location_check","offsetY":-1,
            "predicate":{"block":{"blocks":"minecraft:stone"}}}},
        {"rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:emerald"}],
         "condition":{"type":"minecraft:location_check"}},
        {"rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:apple"}],
         "condition":{"type":"minecraft:location_check",
            "predicate":{"dimension":"minecraft:overworld"}}},
        {"rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:gold_ingot"}],
         "condition":{"type":"minecraft:location_check",
            "predicate":{"position":{"y":{"min":1000}}}}}
    ]}"#;
    with_tables("where", &[("test/where", WHERE)], |game| {
        let mut roll = |origin| {
            names(
                &game
                    .roll_table_at("test:where", origin, None, None)
                    .unwrap(),
            )
            .join(",")
        };
        assert_eq!(roll(None), "");
        assert_eq!(
            roll(Some([2.5, 100.5, 2.5])),
            "minecraft:apple,minecraft:diamond,minecraft:emerald"
        );
        assert_eq!(
            roll(Some([2.5, 101.5, 2.5])),
            "minecraft:apple,minecraft:emerald"
        );
        assert_eq!(
            roll(Some([4000.5, 100.5, 2.5])),
            "minecraft:apple,minecraft:emerald"
        );
    });
}
