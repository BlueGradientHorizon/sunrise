#[test]
#[ignore = "daily"]
fn placement_preview_checks_obstruction_without_changing_the_level() {
    use super::super::DryLevel;
    use sunrise_core::{position::BlockPos, random::LegacyRandom};
    use sunrise_world::{
        level::{Behaviours, LevelAccess, PlaceContext, PlaceOutcome, Ticks, data::load_registry},
        registry::BlockStateId,
    };

    struct Level {
        air: BlockStateId,
        stone: BlockStateId,
        power: BlockStateId,
        client_side: bool,
        obstructed: bool,
        loaded: bool,
        ceiling: i32,
    }
    impl sunrise_world::level::BlockEntityNeighbors for Level {}
    impl LevelAccess for Level {
        fn block_state(&self, pos: BlockPos) -> BlockStateId {
            if pos == BlockPos::new(0, 0, 0) {
                self.stone
            } else if pos == BlockPos::new(-1, 1, 0) {
                self.power
            } else {
                self.air
            }
        }
        fn set_raw(&mut self, _: BlockPos, _: BlockStateId) -> Option<BlockStateId> {
            panic!("preview must not write to the live level");
        }
        fn is_loaded(&self, _: BlockPos) -> bool {
            self.loaded
        }
        fn is_client_side(&self) -> bool {
            self.client_side
        }
        fn max_y(&self) -> i32 {
            self.ceiling
        }
        fn is_unobstructed(&self, pos: BlockPos, _: BlockStateId) -> bool {
            assert_eq!(pos, BlockPos::new(0, 1, 0));
            !self.obstructed
        }
        fn random(&mut self) -> &mut LegacyRandom {
            panic!("preview must not advance the live level's random source");
        }
        fn game_time(&self) -> i64 {
            0
        }
    }

    let resources = std::path::PathBuf::from(
        std::env::var_os("SUNRISE_TEST_RESOURCES")
            .expect("set SUNRISE_TEST_RESOURCES to prepared resources"),
    );
    let registry = load_registry(&resources).unwrap();
    let behaviours = Behaviours::load(&registry, &resources).unwrap();
    impl super::super::dry_level::PredictionBase for Level {}

    let mut base = Level {
        air: registry.default_state("minecraft:air").unwrap(),
        stone: registry.default_state("minecraft:stone").unwrap(),
        power: registry.default_state("minecraft:redstone_block").unwrap(),
        client_side: true,
        obstructed: false,
        loaded: true,
        ceiling: 320,
    };
    for item in [
        "minecraft:stone",
        "minecraft:oak_slab",
        "minecraft:oak_door",
        "minecraft:tnt",
        "minecraft:piston",
        "minecraft:sticky_piston",
        // GrowingPlantHeadBlock.getStateForPlacement draws a random age.
        "minecraft:twisting_vines",
    ] {
        let ctx = PlaceContext {
            clicked_pos: BlockPos::new(0, 0, 0),
            face: sunrise_core::geometry::Direction::Up,
            hit: sunrise_core::geometry::Vec3::new(0.5, 1.0, 0.5),
            inside: false,
            yaw: 0.0,
            pitch: 45.0,
            sneaking: false,
            item: item.into(),
            can_use_game_master_blocks: false,
        };
        for (client_side, obstructed, loaded, ceiling) in [
            (true, false, true, 320),
            (true, true, true, 320),
            (true, false, false, 320),
            (true, false, true, 1),
            (false, false, true, 320),
        ] {
            base.client_side = client_side;
            base.obstructed = obstructed;
            base.loaded = loaded;
            base.ceiling = ceiling;
            let valid = !obstructed && loaded && ceiling > 1;
            let mut preview = DryLevel::new(&base);
            let mut ticks = Ticks::default();
            let outcome = behaviours.place_block(&mut preview, &mut ticks, &ctx);
            assert_eq!(
                matches!(outcome, PlaceOutcome::Placed { .. }),
                valid,
                "{item}, obstructed={obstructed}, loaded={loaded}, ceiling={ceiling}"
            );
            assert_eq!(preview.changed.is_empty(), !valid);
            if valid
                && matches!(
                    item,
                    "minecraft:tnt" | "minecraft:piston" | "minecraft:sticky_piston"
                )
            {
                assert_eq!(
                    registry
                        .state(preview.block_state(BlockPos::new(0, 1, 0)))
                        .unwrap()
                        .name,
                    if item == "minecraft:tnt" && !client_side {
                        "minecraft:air"
                    } else {
                        item
                    },
                    "powered {item}, client_side={client_side}"
                );
                assert_eq!(
                    ticks.block_events.is_empty(),
                    client_side || item == "minecraft:tnt",
                    "only the server starts a powered piston"
                );
            }
            assert_eq!(base.block_state(BlockPos::new(0, 1, 0)), base.air);
        }
    }
}

#[test]
#[ignore = "daily"]
fn redstone_updates_respect_the_preview_level_side() {
    use super::super::{DryLevel, dry_level::PredictionBase};
    use std::collections::HashMap;
    use sunrise_core::{geometry::Direction, position::BlockPos, random::LegacyRandom};
    use sunrise_world::{
        level::{Behaviours, BlockEntityNeighbors, LevelAccess, Ticks, data::load_registry, flags},
        registry::BlockStateId,
    };

    struct Level {
        cells: HashMap<BlockPos, BlockStateId>,
        air: BlockStateId,
        client_side: bool,
    }
    impl BlockEntityNeighbors for Level {}
    impl PredictionBase for Level {}
    impl LevelAccess for Level {
        fn block_state(&self, pos: BlockPos) -> BlockStateId {
            self.cells.get(&pos).copied().unwrap_or(self.air)
        }
        fn set_raw(&mut self, _: BlockPos, _: BlockStateId) -> Option<BlockStateId> {
            panic!("preview must not write to the live level");
        }
        fn is_loaded(&self, _: BlockPos) -> bool {
            true
        }
        fn is_client_side(&self) -> bool {
            self.client_side
        }
        fn random(&mut self) -> &mut LegacyRandom {
            panic!("preview must not advance the live level's random source");
        }
        fn game_time(&self) -> i64 {
            0
        }
    }

    let resources = std::path::PathBuf::from(
        std::env::var_os("SUNRISE_TEST_RESOURCES")
            .expect("set SUNRISE_TEST_RESOURCES to prepared resources"),
    );
    let registry = load_registry(&resources).unwrap();
    let behaviours = Behaviours::load(&registry, &resources).unwrap();
    let default = |block| registry.default_state(block).unwrap();
    let state = |block, props: &[(&str, &str)]| {
        let mut descriptor = registry.state(default(block)).unwrap().clone();
        for &(key, value) in props {
            descriptor.properties.insert(key.into(), value.into());
        }
        registry.id(&descriptor).unwrap()
    };
    let observer = state("minecraft:observer", &[("powered", "true")]);
    let repeater = state(
        "minecraft:repeater",
        &[("facing", "north"), ("locked", "true")],
    );
    let cake = default("minecraft:cake");
    let comparator = default("minecraft:comparator");
    let pos = BlockPos::new(0, 1, 0);
    let west = BlockPos::new(-1, 1, 0);
    let conductor_pos = BlockPos::new(1, 1, 0);
    let comparator_pos = BlockPos::new(2, 1, 0);

    for client_side in [true, false] {
        // Put the unsupported comparator beyond a conductor: ordinary six-way
        // notification and source shape updates must not reach it.
        let base = Level {
            cells: HashMap::from([
                (conductor_pos, default("minecraft:stone")),
                (comparator_pos, comparator),
            ]),
            air: default("minecraft:air"),
            client_side,
        };
        let mut dry = DryLevel::new(&base);
        let mut ticks = Ticks::default();
        assert!(behaviours.set_block(
            &mut dry,
            &mut ticks,
            pos,
            observer,
            flags::UPDATE_MOVE_BY_PISTON | flags::UPDATE_KNOWN_SHAPE,
        ));
        let observer_powered = dry.block_state(pos) == observer;

        let mut dry = DryLevel::new(&base);
        let mut ticks = Ticks::default();
        let repeater_locked = behaviours.update_shape(
            &mut dry,
            &mut ticks,
            repeater,
            pos,
            Direction::West,
            west,
            base.air,
        ) == repeater;

        assert!(behaviours.set_block(
            &mut dry,
            &mut ticks,
            pos,
            cake,
            flags::UPDATE_NEIGHBORS | flags::UPDATE_KNOWN_SHAPE,
        ));
        assert_eq!(dry.block_state(pos), cake);
        let comparator_retained = dry.block_state(comparator_pos) == comparator;

        assert_eq!(
            (observer_powered, repeater_locked, comparator_retained),
            (client_side, client_side, client_side),
            "observer powered, repeater locked, comparator retained (client_side={client_side})"
        );
    }
}
