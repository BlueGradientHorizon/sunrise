

#[cfg(test)]
mod tests {
    use sunrise_gameplay::game::native_entity_refs::tests::with_game;
    use {crate::game::native_world_read_common::*, sunrise_gameplay_common::game::native_world_read_common::{BlockRegistry, BlockStateId, EngineFluidState, FluidKind, Items, World, api_fluid_state, block_in_tag, can_see_sky, fluid_in_tag, fluid_shape, install_block_tags, prepare_block_tags}};
    use mod_api::LevelHost;
    use std::sync::Arc;
    use sunrise_items::items::TagRegistry;
    use sunrise_level::chunk::{Chunk, Light};

    #[test]
    fn sky_visibility_uses_the_published_sky_layer() {
        let mut world = World::new(BlockStateId(0), 0);
        let y = world.shape.min_y;
        let above = world.shape.max_y() + 16;
        let mut chunk = Chunk::empty(0, 0);
        Arc::make_mut(&mut chunk.sections[0]).light = Light::Uniform(0xef);
        world.chunks.insert((0, 0), Arc::new(chunk));
        assert!(
            !can_see_sky(&world, [0, y, 0]),
            "block light is not sky light"
        );
        assert!(can_see_sky(&world, [0, above, 0]));
        assert!(can_see_sky(&world, [32, y, 0]));
        world.shape.sky_light = false;
        assert!(!can_see_sky(&world, [0, above, 0]));
        assert!(!can_see_sky(&world, [32, y, 0]));
        assert!(!world.loaded(32, 0));
    }

    #[test]
    fn a_fluid_shape_is_full_under_the_same_fluid_and_its_own_height_otherwise() {
        let source = EngineFluidState::WATER;
        let shallow = EngineFluidState::new(FluidKind::FlowingWater, 3, false);
        let lava = EngineFluidState::new(FluidKind::Lava, 8, false);
        assert_eq!(fluid_shape(EngineFluidState::EMPTY, Some(source)), None);
        // FlowingFluid.getOwnHeight: amount / 9, so a source is 8/9 high under air.
        let top = |shape: Option<[f64; 6]>| shape.map(|b| b[4]);
        assert_eq!(
            top(fluid_shape(source, None)),
            Some(f64::from(8.0f32 / 9.0))
        );
        assert_eq!(
            top(fluid_shape(shallow, None)),
            Some(f64::from(3.0f32 / 9.0))
        );
        // Flowing water under a source is the same fluid: a full block.
        assert_eq!(top(fluid_shape(shallow, Some(source))), Some(1.0));
        assert_eq!(
            top(fluid_shape(source, Some(lava))),
            Some(f64::from(8.0f32 / 9.0))
        );
        assert_eq!(
            fluid_shape(source, Some(source)),
            Some([0.0, 0.0, 0.0, 1.0, 1.0, 1.0])
        );
    }

    #[test]
    fn fluid_state_conversion_preserves_source_and_empty_invariants() {
        mod_platform::__private::install(Default::default());
        sunrise_gameplay_common::game::api_values::register_game_keys([]);
        for kind in FluidKind::ALL {
            let engine =
                EngineFluidState::new(kind, if kind == FluidKind::Empty { 0 } else { 8 }, true);
            let api = api_fluid_state(engine).unwrap();
            assert_eq!(api.is_empty(), kind == FluidKind::Empty);
            assert_eq!(api.is_source(), engine.is_source());
            assert_eq!(mod_api::__private::fluid_id(api.fluid), Some(kind.name()));
        }
        let source = api_fluid_state(EngineFluidState::new(FluidKind::Water, 8, true)).unwrap();
        let flowing =
            api_fluid_state(EngineFluidState::new(FluidKind::FlowingWater, 8, false)).unwrap();
        assert!(source.falling && source.is_source());
        assert_eq!(flowing.amount, 8);
        assert!(!flowing.is_source());
    }

    #[test]
    fn api_block_tags_rebind_the_merged_snapshot_without_changing_keys() {
        use mod_platform::__private::part_tests;
        mod_platform::__private::install(Default::default());
        sunrise_gameplay_common::game::api_values::register_game_keys([
            "minecraft:stone".into(),
            "test:machine".into(),
        ]);
        let stone = part_tests::block_key("stone");
        let machine = part_tests::block_key("test:machine");
        let mut items = Items::for_tests(Default::default());
        items
            .block_tags
            .insert("test:machines".into(), ["test:machine".into()].into());
        items.block_tags.insert(
            "minecraft:base_stone_overworld".into(),
            ["minecraft:stone".into()].into(),
        );
        for facts in [
            r#"{"defaults":{"minecraft:air":2,"minecraft:stone":0,"test:machine":1},"states":[["s",false,false,"","minecraft:stone"],["m",false,false,"","test:machine"],["a",true,false,"","minecraft:air"]]}"#,
            r#"{"defaults":{"minecraft:air":2,"minecraft:stone":1,"test:machine":0},"states":[["m",false,false,"","test:machine"],["s",false,false,"","minecraft:stone"],["a",true,false,"","minecraft:air"]]}"#,
        ] {
            // Build reordered numeric registries from the same API definitions.
            let registry = BlockRegistry::from_facts_json(facts.as_bytes()).unwrap();
            sunrise_gameplay_common::game::api_values::resolve_block_definitions(&registry).unwrap();
            install_block_tags(prepare_block_tags(&items.block_tags));
            assert_eq!(block_in_tag(machine, "test:machines"), Some(true));
            assert_eq!(block_in_tag(stone, "test:machines"), Some(false));
            assert_eq!(block_in_tag(stone, "base_stone_overworld"), Some(true));
            assert_eq!(
                block_in_tag(machine, "minecraft:base_stone_overworld"),
                Some(false)
            );
            assert_eq!(block_in_tag(machine, "test:missing"), Some(false));
            assert_eq!(machine, part_tests::block_key("test:machine"));
        }
        // A replaced merged tag snapshot must discard previous membership, not union it.
        items
            .block_tags
            .insert("test:machines".into(), ["minecraft:stone".into()].into());
        install_block_tags(prepare_block_tags(&items.block_tags));
        assert_eq!(block_in_tag(machine, "test:machines"), Some(false));
        assert_eq!(block_in_tag(stone, "test:machines"), Some(true));
        // Valid key, no owning world: never answer false using another thread's tags.
        assert_eq!(
            std::thread::spawn(move || block_in_tag(stone, "test:machines"))
                .join()
                .unwrap(),
            None
        );
    }

    #[test]
    fn reads_only_loaded_in_bounds_states() {
        use {sunrise_gameplay::game::server_side::native_world_read_server::NativeBlockReadError, sunrise_gameplay::game::server_side::native_world_read_server::read_block};

        let facts = br#"{"defaults":{"minecraft:air":0,"test:pillar":1},"states":[
            ["air",true,false,"","minecraft:air"],
            ["pillar",false,false,"",{"id":"test:pillar","properties":{"axis":"x"}}]
        ]}"#;
        let registry = BlockRegistry::from_facts_json(&facts[..]).unwrap();
        let mut world = World::new(BlockStateId(0), 0);
        world.chunks.insert((-1, 0), Arc::new(Chunk::empty(-1, 0)));
        world.set(-1, 64, 1, BlockStateId(1)).unwrap();
        // The Nether's build range starts at y 0 (its level's shape).
        let mut nether = World::new(BlockStateId(0), 0);
        nether.shape = sunrise_level::chunk::LevelShape::NETHER;
        nether.chunks.insert((-1, 0), Arc::new(Chunk::empty(-1, 0)));
        let block = read_block(&world, &registry, [-1, 64, 1]).unwrap();
        assert_eq!(block.name, "test:pillar");
        assert_eq!(block.properties["axis"], "x");
        assert_eq!(
            read_block(&world, &registry, [-1, 65, 1]).unwrap().name,
            "minecraft:air"
        );
        assert_eq!(
            read_block(&world, &registry, [0, 64, 1]),
            Err(NativeBlockReadError::Unloaded)
        );
        assert_eq!(
            read_block(&nether, &registry, [-1, -1, 1]),
            Err(NativeBlockReadError::OutOfBounds)
        );
        assert_eq!(
            read_block(&world, &registry, [30_000_000, 64, 1]),
            Err(NativeBlockReadError::OutOfBounds)
        );
        world.set(-1, 64, 1, BlockStateId(99)).unwrap();
        assert_eq!(
            read_block(&world, &registry, [-1, 64, 1]),
            Err(NativeBlockReadError::InvalidState)
        );
        assert_eq!(world.chunks.len(), 1, "reads must not load chunks");
    }

    #[test]
    #[ignore = "daily"]
    fn recipes_of_type_answers_vanilla_types_from_the_live_table() {
        with_game(|game| {
            let smelting = game.recipes_of_type("minecraft:smelting").0;
            let iron = smelting
                .iter()
                .find(|recipe| recipe.id == "minecraft:iron_ingot_from_smelting_raw_iron")
                .expect("vanilla's smelting recipe of raw iron");
            assert_eq!(iron.json["cookingtime"], 200);
            assert_eq!(iron.cooking_time, Some(200));
            assert_eq!(iron.experience, Some(0.7));
            assert_eq!(iron.ingredients.len(), 1);
            assert!(iron.ingredients[0].items.contains("minecraft:raw_iron"));
            assert_eq!(iron.results[0].item.as_ref(), "minecraft:iron_ingot");
            let shaped = game.recipes_of_type("minecraft:crafting").0;
            let bucket = shaped
                .iter()
                .find(|recipe| recipe.id == "minecraft:bucket")
                .expect("vanilla's bucket recipe");
            assert_eq!(bucket.ingredients.len(), 3, "its pattern's three cells");
            assert_eq!(bucket.results[0].item.as_ref(), "minecraft:bucket");
            assert_eq!(bucket.cooking_time, None);
            assert!(
                !game
                    .recipes_of_type("minecraft:campfire_cooking")
                    .0
                    .is_empty()
            );
            assert!(game.recipes_of_type("nope:missing").0.is_empty());
        });
    }

    #[test]
    #[ignore = "daily"]
    fn tag_and_fluid_reads_follow_vanilla_and_load_nothing() {
        with_game(|game| {
            let dir =
                std::env::temp_dir().join(format!("sunrise-tag-reads-{}", std::process::id()));
            game.side.session_lock.0 = Some(sunrise_sim::save::SessionLock::acquire(&dir).unwrap());
            let registry = game.pack.assets.registry;
            let stone = registry.default_state("minecraft:stone").unwrap();
            let water = registry.default_state("minecraft:water").unwrap();
            let slab = game
                .block_state_id("minecraft:oak_slab[waterlogged=true]")
                .unwrap();
            let dry_slab = game.block_state_id("minecraft:oak_slab").unwrap();
            let lava = registry.default_state("minecraft:lava").unwrap();

            // Members come in the tag file's order, in each registry, with or without the
            // namespace; an unknown tag lists nothing.
            let oak = [
                "oak_log",
                "oak_wood",
                "stripped_oak_log",
                "stripped_oak_wood",
            ]
            .map(|name| format!("minecraft:{name}"));
            let engine_members = |registry, tag: &str| {
                game.native_tag_items().map(|items| {
                    items
                        .tag_members(registry, &sunrise_command::args::normalized(tag))
                        .unwrap_or_default()
                        .to_vec()
                })
            };
            for registry in [TagRegistry::Block, TagRegistry::Item] {
                assert_eq!(
                    engine_members(registry, "oak_logs").as_deref(),
                    Some(&oak[..])
                );
            }
            {
                let host: &dyn mod_api::Host = game;
                let tags = host.tags();
                let oak_tag = tags
                    .key(&mod_api::Identifier::parse("minecraft:oak_logs").unwrap())
                    .unwrap();
                let mod_api::TagMembers::Members(keys) = tags.items(oak_tag) else {
                    panic!("the oak logs tag is loaded");
                };
                assert_eq!(
                    keys.into_iter()
                        .map(|key| key.id(host))
                        .collect::<Option<Vec<_>>>()
                        .as_deref(),
                    Some(&oak.each_ref().map(String::as_str)[..])
                );
                let item = host
                    .item_key(&mod_api::Identifier::parse("minecraft:oak_log").unwrap())
                    .unwrap();
                assert_eq!(tags.is_in_tag(item, oak_tag), Some(true));
            }
            assert_eq!(
                engine_members(TagRegistry::Fluid, "minecraft:water").unwrap(),
                ["minecraft:water", "minecraft:flowing_water"]
            );
            assert_eq!(
                engine_members(TagRegistry::Item, "nope:missing"),
                Some(Vec::new())
            );

            // A full block is one unit box, a bottom slab half of it, air none, and an id no
            // state has is unknown.
            assert_eq!(
                game.state_outline(stone),
                Some(vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]])
            );
            assert_eq!(
                game.state_outline(dry_slab),
                Some(vec![[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]])
            );
            let air = registry.default_state("minecraft:air").unwrap();
            assert_eq!(game.state_outline(air), Some(Vec::new()));
            assert_eq!(game.state_outline(BlockStateId(u32::MAX)), None);

            assert_eq!(
                game.block_in_tag(
                    mod_platform::__private::part_tests::block_key("minecraft:stone"),
                    "minecraft:base_stone_overworld"
                ),
                Some(true)
            );
            assert_eq!(
                game.block_in_tag(
                    mod_platform::__private::part_tests::block_key("stone"),
                    "base_stone_overworld"
                ),
                Some(true)
            );
            assert_eq!(
                game.block_in_tag(
                    mod_platform::__private::part_tests::block_key("minecraft:stone"),
                    "minecraft:logs"
                ),
                Some(false)
            );
            assert_eq!(
                game.block_in_tag(
                    mod_platform::__private::part_tests::block_key("minecraft:stone"),
                    "nope:missing"
                ),
                Some(false)
            );

            assert_eq!(
                game.state_fluid_state(water),
                api_fluid_state(EngineFluidState::WATER)
            );
            assert_eq!(
                game.fluid_in_tag("minecraft:water", "minecraft:water"),
                Some(true)
            );
            assert_eq!(game.fluid_in_tag("water", "water"), Some(true));
            assert_eq!(
                game.fluid_in_tag("minecraft:water", "minecraft:lava"),
                Some(false)
            );
            assert_eq!(
                mod_api::__private::fluid_id(game.state_fluid_state(lava).unwrap().fluid).unwrap(),
                "minecraft:lava"
            );
            assert_eq!(
                game.fluid_in_tag("minecraft:lava", "minecraft:lava"),
                Some(true)
            );
            assert_eq!(
                game.fluid_in_tag("minecraft:lava", "minecraft:water"),
                Some(false)
            );
            assert_eq!(
                game.fluid_in_tag("minecraft:empty", "minecraft:water"),
                Some(false)
            );
            assert_eq!(
                game.state_fluid_state(slab),
                api_fluid_state(EngineFluidState::WATER)
            );
            assert_eq!(
                game.state_fluid_state(dry_slab),
                api_fluid_state(EngineFluidState::EMPTY)
            );
            assert_eq!(
                game.state_fluid_state(stone),
                api_fluid_state(EngineFluidState::EMPTY)
            );
            assert_eq!(game.state_fluid_state(BlockStateId(u32::MAX)), None);

            // The world holds a chunk with water in it; a neighbour is not loaded.
            let mut chunk = Chunk::empty(0, 0);
            chunk.set(1, 64, 1, slab.0);
            game.world.chunks.insert((0, 0), Arc::new(chunk));
            let level = game.level(game.dim.id()).unwrap();
            let chunks = game.world.chunks.len();
            assert_eq!(
                game.level_fluid_state(level, [1, 64, 1]),
                api_fluid_state(EngineFluidState::WATER)
            );
            assert_eq!(
                game.level_fluid_state(level, [2, 64, 1]),
                api_fluid_state(EngineFluidState::EMPTY)
            );
            assert_eq!(game.level_fluid_state(level, [16, 64, 1]), None);
            assert_eq!(game.level_fluid_state(level, [1, 4000, 1]), None);
            game.options.execution_role = sunrise_gameplay_common::game::execution_role::ExecutionRole::Client;
            game.with_level(|_, host, _| {
                // Not an authoritative level: tags are not the server's, so they read None.
                assert_eq!(
                    host.block_in_tag(
                        mod_platform::__private::part_tests::block_key("minecraft:stone"),
                        "minecraft:base_stone_overworld"
                    ),
                    None
                );
                assert_eq!(
                    host.fluid_in_tag("minecraft:water", "minecraft:water"),
                    None
                );
                assert_eq!(
                    host.fluid_state([1, 64, 1]),
                    api_fluid_state(EngineFluidState::WATER)
                );
            });
            game.options.execution_role =
                sunrise_gameplay_common::game::execution_role::ExecutionRole::IntegratedServer;
            game.options.dedicated = Some(Vec::new());
            let bounds = game.level_height(level).unwrap();
            use sunrise_gameplay_common::game::api_values::TryIntoApi;
            let stone = stone.try_into_api().unwrap();
            let slab = slab.try_into_api().unwrap();
            let invalid = mod_api::__private::block_state_from_ordinal(stone.block(), u16::MAX);
            let parsed = game
                .block_state_id("minecraft:oak_slab[waterlogged=true]")
                .try_into_api()
                .unwrap();
            let default = game
                .block_state_default(mod_platform::__private::part_tests::block_key(
                    "minecraft:oak_slab",
                ))
                .try_into_api()
                .unwrap();
            game.with_level(|_, host, ticks| {
                assert_eq!(
                    host.block_state_id("minecraft:oak_slab[waterlogged=true]"),
                    parsed
                );
                assert_eq!(
                    host.block_state_default(mod_platform::__private::part_tests::block_key(
                        "minecraft:oak_slab"
                    )),
                    default
                );
                assert_eq!(host.block_state_id("minecraft:oak_slab[bad=true]"), None);

                let wrapped = sunrise_gameplay_common::game::mod_api_host::LevelTicks::new(&*host, ticks);
                assert_eq!(
                    wrapped.block_state_id("minecraft:oak_slab[waterlogged=true]"),
                    parsed
                );
                assert_eq!(
                    wrapped.block_state_default(mod_platform::__private::part_tests::block_key(
                        "minecraft:oak_slab"
                    )),
                    default
                );
                assert_eq!(wrapped.block_state_id("minecraft:oak_slab[bad=true]"), None);

                assert_eq!(host.state_string(stone).as_deref(), Some("minecraft:stone"));
                assert!(
                    host.state_string(slab)
                        .unwrap()
                        .contains("waterlogged=true")
                );
                assert_eq!(host.state_string(invalid), None);
                assert_eq!((host.min_y(), host.height()), bounds);
                assert_eq!(
                    host.block_in_tag(
                        mod_platform::__private::part_tests::block_key("minecraft:stone"),
                        "minecraft:base_stone_overworld"
                    ),
                    Some(true)
                );
                assert_eq!(
                    host.block_in_tag(
                        mod_platform::__private::part_tests::block_key("minecraft:stone"),
                        "minecraft:logs"
                    ),
                    Some(false)
                );
                assert_eq!(
                    host.fluid_state([1, 64, 1]),
                    api_fluid_state(EngineFluidState::WATER)
                );
                assert_eq!(host.fluid_state([16, 64, 1]), None);
                assert_eq!(
                    host.state_fluid_state(slab),
                    api_fluid_state(EngineFluidState::WATER)
                );
                assert_eq!(
                    host.fluid_in_tag("minecraft:water", "minecraft:water"),
                    Some(true)
                );
                assert_eq!(
                    host.fluid_in_tag("minecraft:empty", "minecraft:water"),
                    Some(false)
                );
            });
            {
                use mod_api::LevelHost;
                let hit =
                    sunrise_gameplay_common::game::native_hooks::HitLevel(sunrise_sim::health::DamageLevel::of(
                        &mut game.world,
                        game.pack.assets.registry,
                        Some(world_items!(game)),
                        Some(&game.pack.block_registries),
                        &game.damage_hooks,
                    ));
                assert_eq!(
                    hit.block_state_id("minecraft:oak_slab[waterlogged=true]"),
                    parsed
                );
                assert_eq!(
                    hit.block_state_default(mod_platform::__private::part_tests::block_key(
                        "minecraft:oak_slab"
                    )),
                    default
                );
                assert_eq!(hit.block_state_id("minecraft:oak_slab[bad=true]"), None);

                assert_eq!(hit.state_string(stone).as_deref(), Some("minecraft:stone"));
                assert!(hit.state_string(slab).unwrap().contains("waterlogged=true"));
                assert_eq!(hit.state_string(invalid), None);
                assert_eq!((hit.min_y(), hit.height()), bounds);
                assert_eq!(
                    hit.block_in_tag(
                        mod_platform::__private::part_tests::block_key("minecraft:stone"),
                        "minecraft:base_stone_overworld"
                    ),
                    Some(true)
                );
                assert_eq!(
                    hit.fluid_in_tag("minecraft:water", "minecraft:water"),
                    Some(true)
                );
            }
            {
                use mod_api::LevelHost;
                let hit =
                    sunrise_gameplay_common::game::native_hooks::HitLevel(sunrise_sim::health::DamageLevel::of(
                        &mut game.world,
                        game.pack.assets.registry,
                        None,
                        None,
                        &game.damage_hooks,
                    ));
                assert_eq!(hit.block_state_id("minecraft:stone"), None);
                assert_eq!(
                    hit.block_state_default(mod_platform::__private::part_tests::block_key(
                        "minecraft:stone"
                    )),
                    None
                );
                assert_eq!(
                    hit.block_in_tag(
                        mod_platform::__private::part_tests::block_key("minecraft:stone"),
                        "minecraft:logs"
                    ),
                    None
                );
                assert_eq!(hit.fluid_in_tag("minecraft:water", "minecraft:water"), None);
            }
            assert_eq!(game.world.chunks.len(), chunks, "nothing was loaded");
            game.options.execution_role = sunrise_gameplay_common::game::execution_role::ExecutionRole::Client;
            let host: &dyn mod_api::Host = game;
            let tags = host.tags();
            let oak_tag = tags
                .key(&mod_api::Identifier::parse("minecraft:oak_logs").unwrap())
                .unwrap();
            let item = host
                .item_key(&mod_api::Identifier::parse("minecraft:oak_log").unwrap())
                .unwrap();
            assert_eq!(tags.items(oak_tag), mod_api::TagMembers::Unavailable);
            assert_eq!(tags.is_in_tag(item, oak_tag), None);
            game.side.session_lock.0 = None;
            let _ = std::fs::remove_dir_all(dir);
        });
    }

    /// Sources and their flowing forms are one fluid, the empty fluid has no bucket and no
    /// pickup sound, and a name no fluid has answers nothing, through a mod's `Host`.
    #[test]
    #[ignore = "daily"]
    fn fluid_sameness_buckets_and_bucket_sounds_follow_vanilla() {
        with_game(|game| {
            let host: &dyn mod_api::Host = game;
            assert!(host.fluid_is_same("minecraft:water", "minecraft:flowing_water"));
            assert!(host.fluid_is_same("flowing_lava", "lava"));
            assert!(host.fluid_is_same("minecraft:empty", "minecraft:empty"));
            assert!(!host.fluid_is_same("minecraft:water", "minecraft:lava"));
            assert!(!host.fluid_is_same("minecraft:water", "minecraft:empty"));
            assert!(!host.fluid_is_same("nope:missing", "nope:missing"));

            assert_eq!(
                host.fluid_bucket("minecraft:flowing_water")
                    .and_then(mod_api::__private::item_key_id),
                Some("minecraft:water_bucket")
            );
            assert_eq!(
                host.fluid_bucket("lava")
                    .and_then(mod_api::__private::item_key_id),
                Some("minecraft:lava_bucket")
            );
            assert_eq!(host.fluid_bucket("minecraft:empty"), None);
            assert_eq!(host.fluid_bucket("nope:missing"), None);

            assert_eq!(
                host.fluid_pickup_sound("minecraft:water"),
                Some("minecraft:item.bucket.fill")
            );
            assert_eq!(
                host.fluid_pickup_sound("minecraft:flowing_lava"),
                Some("minecraft:item.bucket.fill_lava")
            );
            assert_eq!(host.fluid_pickup_sound("minecraft:empty"), None);
            assert_eq!(host.fluid_pickup_sound("nope:missing"), None);

            assert_eq!(
                host.fluid_empty_sound("flowing_water"),
                Some("minecraft:item.bucket.empty")
            );
            assert_eq!(
                host.fluid_empty_sound("minecraft:lava"),
                Some("minecraft:item.bucket.empty_lava")
            );
            assert_eq!(host.fluid_empty_sound("minecraft:empty"), None);
            assert_eq!(host.fluid_empty_sound("nope:missing"), None);
        });
    }
}
use sunrise_gameplay_common::game::tables::GameItems as _;

use sunrise_gameplay_common::game::tables::WorldTablesExt as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_level_refs_common::NativeLevelRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_world_read_common::NativeWorldReadGameExt1 as _;
