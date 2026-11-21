use super::*;
use crate::game::native_entity_refs::tests::{
    bind_local_player, run_fresh_cases, with_game, with_game_at,
};
use mod_api::{
    BlockEdit, BlockEntityPart, BlockEntityTickEvent, BlockPart, BlockRun, BlockUpdateFlags,
    Direction, EntityJoinLevelEvent, Flow, Hooks, HostTransaction, ItemResource, ItemSpawnEvent,
    LevelHost, ModEntry, NeighborNotifyEvent, Registrar, STORAGE,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};
use sunrise_client::{
    chunk::Chunk,
    containers::{ModBlockEntityFields, ModContainer},
    items::ItemStack,
    motion::PlayerMotion,
};
use sunrise_core::geometry::Vec3;

const POS: Pos = (1, 64, 1);
const CHUNK: (i32, i32) = (0, 0);
const AT: [i32; 3] = [POS.0, POS.1, POS.2];
const KIND: &str = "view_tick:subscribed";
const UNSUBSCRIBED: &str = "view_tick:unsubscribed";

fn row(kind: &str, value: i32) -> BlockEntity {
    BlockEntity::Mod(Box::new(EngineBlockEntity::new(
        kind.into(),
        ModBlockEntityFields {
            data: vec![value],
            ..Default::default()
        },
    )))
}

fn place_subject(game: &mut Game<'_>, kind: &str) -> BlockEntityRef {
    game.world
        .chunks
        .insert(CHUNK, Arc::new(Chunk::empty(0, 0)));
    let stone = game
        .pack
        .assets
        .registry
        .default_state("minecraft:stone")
        .unwrap();
    game.world.set(POS.0, POS.1, POS.2, stone).unwrap();
    game.world.block_entities.insert(POS, row(kind, 23));
    game.world.modified.clear();
    let level = game.level(game.dim.id()).unwrap();
    game.block_entity(level, [POS.0, POS.1, POS.2]).unwrap()
}

fn access<'a, 'g>(game: &'a mut Game<'g>, subject: BlockEntityRef) -> TickAccess<'a, 'g> {
    let state = game.world.block(POS.0, POS.1, POS.2);
    TickAccess {
        game,
        subject,
        state,
    }
}

#[test]
#[ignore = "daily"]
fn retaking_the_view_checks_the_instance_and_state() {
    with_game(|game| {
        let subject = place_subject(game, KIND);
        let mut access = access(game, subject);
        let mut scope = BlockEntityTickScope::new(&mut access);
        assert_eq!(scope.with_view(|view| view.data_slots()[0]), Some(23));
        let host = scope.host();
        let level = subject.level(host).unwrap();
        let pos = subject.position(host).unwrap();
        let glass = access
            .game
            .pack
            .assets
            .registry
            .default_state("minecraft:glass")
            .unwrap();
        access
            .game
            .world
            .set(pos[0], pos[1], pos[2], glass)
            .unwrap();
        assert_eq!(access.game.block_entity(level, pos), Some(subject));
        assert_eq!(
            BlockEntityTickScope::new(&mut access).with_view(|_| ()),
            None
        );
        access
            .game
            .world
            .set(pos[0], pos[1], pos[2], access.state)
            .unwrap();
        assert_eq!(
            BlockEntityTickScope::new(&mut access).with_view(|_| ()),
            Some(())
        );
        access.game.world.block_entities.remove(&POS);
        assert_eq!(
            BlockEntityTickScope::new(&mut access).with_view(|_| ()),
            None
        );
        access.game.world.block_entities.insert(POS, row(KIND, 99));
        assert_eq!(
            BlockEntityTickScope::new(&mut access).with_view(|_| ()),
            None
        );
    });
}

#[test]
#[ignore = "daily"]
fn read_only_and_noop_loans_do_not_journal_or_dirty() {
    with_game(|game| {
        let subject = place_subject(game, KIND);
        let result: Result<(), ()> = game.transaction(|game, _| {
            let mut access = access(game, subject);
            BlockEntityTickScope::new(&mut access)
                .with_view(|mut view| {
                    assert_eq!(view.data_slots(), [23]);
                    assert!(!view.set_data_slot(0, 23));
                    assert!(!view.set_data_slot(1, 24));
                })
                .unwrap();
            assert_eq!(game.journal_kept(), (0, 0));
            Ok(())
        });
        assert_eq!(result, Ok(()));
        assert!(game.world.modified.is_empty());
    });
}

#[test]
#[ignore = "daily"]
fn a_scope_transaction_aborts_subject_and_dropped_item_edits_together() {
    with_game(|game| {
        let subject = place_subject(game, KIND);
        game.spawn_item(
            sunrise_client::items::ItemStack::new("minecraft:stone", 5),
            [1.0, 64.0, 1.0],
            [0.0; 3],
            0,
        );
        let drop_id = game.dropped[0].id;
        let item = game.item_entity(game.entity(drop_id).unwrap()).unwrap();
        let count = item.with_stack(game, |stack| stack.count()).unwrap();
        game.world.modified.clear();
        let mut access = access(game, subject);
        let mut scope = BlockEntityTickScope::new(&mut access);
        let result: Result<(), ()> = scope.transaction(|scope, _| {
            scope
                .with_view(|mut view| {
                    assert!(view.set_data_slot(0, 24));
                    assert!(view.set_nbt("recipe".into(), Some(Tag::Int(6))));
                })
                .ok_or(())?;
            let host = scope.host();
            let rest = item
                .with_stack(host, |stack| stack.project())
                .flatten()
                .unwrap()
                .shrink(1)
                .unwrap();
            assert!(item.set_stack(host, rest).unwrap());
            assert_eq!(
                item.with_stack(host, |stack| stack.count()),
                Some(count - 1)
            );
            scope
                .with_view(|view| assert_eq!(view.data_slots(), [24]))
                .ok_or(())?;
            Err(())
        });
        assert_eq!(result, Err(()));
        scope
            .with_view(|view| {
                assert_eq!(view.data_slots(), [23]);
                assert!(view.nbt().is_empty());
            })
            .unwrap();
        assert_eq!(
            item.with_stack(scope.host(), |stack| stack.count()),
            Some(count)
        );
    });
}

fn check_edit_then_replace(commit: bool) {
    with_game(|game| {
        let subject = place_subject(game, KIND);
        let original = game.world.block(POS.0, POS.1, POS.2);
        let replacement = game
            .pack
            .assets
            .registry
            .default_state("minecraft:glass")
            .unwrap();
        let replacement_state = replacement.try_into_api().unwrap();
        game.world.block_entities.set_mod_containers(
            [(
                original.0,
                Arc::new(ModContainer {
                    slots: 2,
                    ..Default::default()
                }),
            )]
            .into_iter()
            .collect(),
        );
        let taken = ItemStack::new("minecraft:diamond", 1);
        let left = ItemStack::new("minecraft:iron_ingot", 1);
        let BlockEntity::Mod(row) = game.world.block_entities.get_mut(&POS).unwrap() else {
            panic!("expected a mod container");
        };
        row.items = vec![Some(taken.clone()), Some(left.clone())];
        game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
        bind_local_player(game, 20, 123);
        let player = game.player(123).unwrap();
        game.selected = 0;
        assert!(game.inventory[0].is_none());

        let mut access = access(game, subject);
        let mut scope = BlockEntityTickScope::new(&mut access);
        let result = scope.transaction(|scope, _| {
            let (level, pos, stack) = scope
                .with_view(|mut view| {
                    let stack = view.item_slot(0).unwrap().to_owned().unwrap();
                    assert_eq!(view.set_item_slot(0, None), Ok(true));
                    (view.level(), view.position(), stack)
                })
                .unwrap();
            let host = scope.host();
            assert_eq!(player.give_stack(host, stack), Ok(None));
            // The child keeps the row it removed; the parent has the earlier snapshot.
            host.transaction(|host, _| {
                assert_eq!(
                    level.set_block(
                        host,
                        pos,
                        replacement_state,
                        BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::KNOWN_SHAPE
                    ),
                    Ok(true)
                );
                Ok::<(), ()>(())
            })?;
            assert_eq!(scope.with_view(|_| ()), None);
            if commit { Ok(()) } else { Err(()) }
        });
        assert_eq!(result, if commit { Ok(()) } else { Err(()) });
        assert_eq!(
            scope.with_view(|view| view.handle()),
            (!commit).then_some(subject)
        );
        game.flush_spill();

        if commit {
            assert_eq!(game.world.block(POS.0, POS.1, POS.2), replacement);
            assert!(game.world.block_entities.get(&POS).is_none());
            assert_eq!(game.inventory[0], Some(taken));
            let [drop] = game.dropped.as_slice() else {
                panic!("expected exactly the item left in the replaced container");
            };
            assert_eq!(drop.stack, left);
        } else {
            assert_eq!(game.world.block(POS.0, POS.1, POS.2), original);
            assert_eq!(
                game.world.block_entities.get(&POS).unwrap().slots(),
                [Some(taken), Some(left)]
            );
            assert!(game.inventory[0].is_none());
            assert!(game.dropped.is_empty());
        }
    });
}

#[test]
#[ignore = "daily"]
fn edit_then_replace_commit_spills_only_the_remaining_contents() {
    const NAME: &str = "game::native_hooks::block_entity_tick::tests::edit_then_replace_commit_spills_only_the_remaining_contents";
    if run_fresh_cases(NAME, "SUNRISE_RESIDENT_VIEW_REMOVAL", &["run"]) {
        return;
    }
    #[derive(Debug, PartialEq)]
    enum Seen {
        Join { row_present: bool },
        Spawn,
        Neighbor { row_present: bool },
    }
    struct Listener(Arc<Mutex<Vec<Seen>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Listener {
        fn entity_join_level(
            &self,
            host: &mut dyn ServerHost,
            event: &mut EntityJoinLevelEvent<'_>,
        ) -> Flow {
            if event.kind == "minecraft:item" {
                let level = host.level("minecraft:overworld").unwrap();
                self.0.lock().unwrap().push(Seen::Join {
                    row_present: level.block_entity(host, AT).is_some(),
                });
            }
            Flow::Continue
        }
        fn item_spawn(&self, _: &mut dyn ServerHost, _: &mut ItemSpawnEvent<'_>) {
            self.0.lock().unwrap().push(Seen::Spawn);
        }
        fn neighbor_notify(&self, host: &dyn LevelHost, e: &mut NeighborNotifyEvent) -> Flow {
            if e.pos == mod_api::BlockPos::new(POS.0, POS.1, POS.2) {
                self.0.lock().unwrap().push(Seen::Neighbor {
                    row_present: host.block_entity(AT).is_some(),
                });
            }
            Flow::Continue
        }
    }
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mods: mod_api::Mods = vec![ModEntry::new(
        "removal_order",
        Box::new(Listener(observed.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || check_edit_then_replace(true));
    assert_eq!(
        *observed.lock().unwrap(),
        [
            Seen::Join { row_present: true },
            Seen::Spawn,
            Seen::Neighbor { row_present: false },
        ]
    );
}

#[test]
#[ignore = "daily"]
fn edit_then_replace_abort_restores_the_original_contents() {
    check_edit_then_replace(false);
}

#[test]
#[ignore = "daily"]
fn reentrant_removal_does_not_spill_the_same_saved_pot_twice() {
    const NAME: &str = "game::native_hooks::block_entity_tick::tests::reentrant_removal_does_not_spill_the_same_saved_pot_twice";
    if run_fresh_cases(NAME, "SUNRISE_RESIDENT_VIEW_REENTRANT_REMOVAL", &["run"]) {
        return;
    }

    struct Listener(std::sync::atomic::AtomicBool);
    #[mod_api::__subscriptions]
    impl Hooks for Listener {
        fn entity_join_level(
            &self,
            host: &mut dyn ServerHost,
            event: &mut EntityJoinLevelEvent<'_>,
        ) -> Flow {
            if event.kind == "minecraft:item" && !self.0.swap(true, Ordering::Relaxed) {
                let level = host.level("minecraft:overworld").unwrap();
                // Re-place a pot and remove it while the first pot is spilling its contents.
                let pot = level.block_state(host, [2, 64, 1]).unwrap();
                assert_eq!(
                    level.set_block(host, AT, pot, BlockUpdateFlags::NONE),
                    Ok(true)
                );
                assert!(level.remove_block(host, AT, false));
            }
            Flow::Continue
        }
    }

    let mods: mod_api::Mods = vec![ModEntry::new(
        "removal_reentry",
        Box::new(Listener(false.into())),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_game(|game| {
            use sunrise_core::nbt::Tag;

            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert(CHUNK, Arc::new(Chunk::empty(0, 0)));
            let registry = game.pack.assets.registry;
            let pot = registry.default_state("minecraft:decorated_pot").unwrap();
            let stone = registry.default_state("minecraft:stone").unwrap();
            game.world.set(POS.0, POS.1, POS.2, pot).unwrap();
            game.world.set(2, 64, 1, pot).unwrap();
            let stock = ItemStack::new("minecraft:diamond", 64);
            let item = sunrise_client::save::item_to_nbt(&stock, game.res.items());
            crate::game::spill::put_saved_entity(
                &mut game.world,
                AT,
                Tag::compound([
                    ("id", Tag::String("minecraft:decorated_pot".into())),
                    ("x", Tag::Int(POS.0)),
                    ("y", Tag::Int(POS.1)),
                    ("z", Tag::Int(POS.2)),
                    ("item", item),
                ]),
            );
            let level = game.level("minecraft:overworld").unwrap();
            let seal = game.transaction_open();
            assert_eq!(
                game.level_set_block(level, AT, stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            game.transaction_close(&seal, true);
            game.flush_spill();

            assert_eq!(game.world.block(POS.0, POS.1, POS.2), game.world.air);
            assert!(crate::game::spill::saved_entity(&game.world, AT).is_none());
            assert!(game.dropped.len() > 1);
            assert!(
                game.dropped
                    .iter()
                    .all(|drop| drop.stack.item == stock.item)
            );
            let spilled: u32 = game.dropped.iter().map(|drop| drop.stack.count).sum();
            assert_eq!(spilled, 64);
        });
    });
}

#[test]
#[ignore = "daily"]
fn spill_join_keeps_the_replacement_furnace_and_its_contents() {
    const NAME: &str = "game::native_hooks::block_entity_tick::tests::spill_join_keeps_the_replacement_furnace_and_its_contents";
    if run_fresh_cases(NAME, "SUNRISE_RESIDENT_VIEW_REPLACEMENT_FURNACE", &["run"]) {
        return;
    }

    const LABEL: &str = "Restocked furnace";
    struct Listener {
        replacement: Arc<Mutex<Option<BlockEntityRef>>>,
        lock: Compound,
    }
    #[mod_api::__subscriptions]
    impl Hooks for Listener {
        fn entity_join_level(
            &self,
            host: &mut dyn ServerHost,
            event: &mut EntityJoinLevelEvent<'_>,
        ) -> Flow {
            if event.kind == "minecraft:item" {
                let level = host.level("minecraft:overworld").unwrap();
                let furnace = level.block_state(host, AT).unwrap();
                let stone = host.block_state_id("minecraft:stone").unwrap();
                // Install a new row, but leave the state the outer write was already setting.
                assert_eq!(
                    level.set_block(host, AT, stone, BlockUpdateFlags::NONE),
                    Ok(true)
                );
                assert_eq!(
                    level.set_block(host, AT, furnace, BlockUpdateFlags::NONE),
                    Ok(true)
                );
                let replacement = level.block_entity(host, AT).unwrap();
                let storage = level
                    .capability(host, &STORAGE, AT, Some(Direction::Up))
                    .unwrap();
                let stock = ItemStack::new("minecraft:raw_iron", 1)
                    .try_into_api()
                    .unwrap();
                host.transaction(|host, tx| {
                    assert_eq!(storage.insert(host, tx, &ItemResource::of(&stock), 7), 7);
                    assert_eq!(replacement.set_name(host, Some(LABEL)), Ok(true));
                    assert_eq!(
                        replacement.set_lock(host, Some(self.lock.clone())),
                        Ok(true)
                    );
                    Ok::<_, ()>(())
                })
                .unwrap();
                *self.replacement.lock().unwrap() = Some(replacement);
            }
            Flow::Continue
        }
    }

    let replacement = Arc::new(Mutex::new(None));
    let lock = Compound::from([("items".into(), Tag::String("minecraft:stick".into()))]);
    let mods = vec![ModEntry::new(
        "removal_replacement",
        Box::new(Listener {
            replacement: replacement.clone(),
            lock: lock.clone(),
        }),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert(CHUNK, Arc::new(Chunk::empty(0, 0)));
            let registry = game.pack.assets.registry;
            let chest = registry.default_state("minecraft:chest").unwrap();
            let furnace = registry.default_state("minecraft:furnace").unwrap();
            game.world.set(POS.0, POS.1, POS.2, chest).unwrap();
            let spilled = ItemStack::new("minecraft:diamond", 1);
            let mut row = BlockEntity::for_block("minecraft:chest").unwrap();
            row.slots_mut()[0] = Some(spilled.clone());
            game.world.block_entities.insert(POS, row);
            let level = game.level("minecraft:overworld").unwrap();
            let original = game.block_entity(level, AT).unwrap();

            assert_eq!(
                game.level_set_block(level, AT, furnace, BlockUpdateFlags::NONE),
                Ok(true)
            );

            let replacement = replacement
                .lock()
                .unwrap()
                .expect("listener installed a furnace");
            assert_ne!(replacement, original);
            assert_eq!(game.block_entity(level, AT), Some(replacement));
            assert_eq!(game.world.block(POS.0, POS.1, POS.2), furnace);
            let row = game.world.block_entities.get(&POS).unwrap();
            assert!(matches!(row, BlockEntity::Furnace(_)));
            assert_eq!(
                row.slots(),
                [Some(ItemStack::new("minecraft:raw_iron", 7)), None, None]
            );
            assert_eq!(replacement.name(game).as_deref(), Some(LABEL));
            assert_eq!(replacement.lock(game), Some(&lock));
            let [drop] = game.dropped.as_slice() else {
                panic!("expected exactly one drop, from the original chest");
            };
            assert_eq!(drop.stack, spilled);
        });
    });
}

#[test]
#[ignore = "daily"]
fn strict_bulk_replacement_aborts_the_block_handle_and_inventory_together() {
    with_game(|game| {
        game.options.dedicated = Some(Vec::new());
        let subject = place_subject(game, KIND);
        let original = game.world.block(POS.0, POS.1, POS.2);
        let chest = game
            .pack
            .assets
            .registry
            .default_state("minecraft:chest")
            .unwrap();
        game.world.block_entities.set_mod_containers(
            [(
                original.0,
                Arc::new(ModContainer {
                    slots: 1,
                    ..Default::default()
                }),
            )]
            .into_iter()
            .collect(),
        );
        let stack = ItemStack::new("minecraft:diamond", 7);
        let BlockEntity::Mod(row) = game.world.block_entities.get_mut(&POS).unwrap() else {
            panic!("expected a mod container");
        };
        row.items = vec![Some(stack.clone())];
        let edit = BlockEdit {
            palette: vec![chest.try_into_api().unwrap()],
            filters: Vec::new(),
            runs: vec![BlockRun {
                x: POS.0,
                y: POS.1,
                z: POS.2,
                height: 1,
                surface: None,
                block: 0,
                only: None,
            }],
        };
        game.world.block_log = Some(Vec::new());
        let mut access = access(game, subject);
        let mut scope = BlockEntityTickScope::new(&mut access);
        let result: Result<(), ()> = scope.transaction(|tick, _| {
            tick.with_view(|mut view| assert_eq!(view.set_item_slot(0, None), Ok(true)))
                .unwrap();
            let host = tick.host();
            assert_eq!(host.edit_blocks("minecraft:overworld", &edit), Ok(1));
            let level = host.level("minecraft:overworld").unwrap();
            assert_ne!(level.block_entity(host, AT), Some(subject));
            assert!(level.block_entity(host, AT).is_some());
            Err(())
        });
        assert_eq!(result, Err(()));
        let level = game.level("minecraft:overworld").unwrap();
        assert_eq!(game.world.block(POS.0, POS.1, POS.2), original);
        assert_eq!(game.block_entity(level, AT), Some(subject));
        assert_eq!(
            game.world.block_entities.get(&POS).unwrap().slots(),
            [Some(stack)]
        );
        assert!(game.dropped.is_empty());
        assert!(game.world.bulk_log.is_empty());
        assert!(game.world.modified.is_empty());
    });
}

#[test]
#[ignore = "daily"]
fn strict_bulk_commit_publishes_whole_chunks_without_drops() {
    with_game(|game| {
        game.options.dedicated = Some(Vec::new());
        let subject = place_subject(game, KIND);
        let chest = game
            .pack
            .assets
            .registry
            .default_state("minecraft:chest")
            .unwrap();
        let edit = BlockEdit {
            palette: vec![chest.try_into_api().unwrap()],
            filters: Vec::new(),
            runs: vec![BlockRun {
                x: POS.0,
                y: POS.1,
                z: POS.2,
                height: 1,
                surface: None,
                block: 0,
                only: None,
            }],
        };
        game.world.block_log = Some(Vec::new());
        let seal = game.transaction_open();
        assert_eq!(game.edit_blocks("minecraft:overworld", &edit), Ok(1));
        assert!(game.world.bulk_log.is_empty());
        game.transaction_close(&seal, true);
        let level = game.level("minecraft:overworld").unwrap();
        assert_eq!(game.world.block(POS.0, POS.1, POS.2), chest);
        let resident = game.block_entity(level, AT).unwrap();
        assert_ne!(resident, subject);
        assert!(matches!(
            game.world.block_entities.get(&POS),
            Some(BlockEntity::Chest(_))
        ));
        assert!(game.dropped.is_empty());
        assert!(game.world.bulk_log.contains(&CHUNK));
        assert!(game.world.block_log.as_ref().unwrap().is_empty());
    });
}

#[test]
#[ignore = "daily"]
fn ordinary_replacement_between_strict_edits_uses_its_own_resident_row() {
    for strict_first in [false, true] {
        with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            let subject = place_subject(game, KIND);
            let original = game.world.block(POS.0, POS.1, POS.2);
            let registry = game.pack.assets.registry;
            let chest = registry.default_state("minecraft:chest").unwrap();
            let glass = registry.default_state("minecraft:glass").unwrap();
            let level = game.level("minecraft:overworld").unwrap();
            let edit = BlockEdit {
                palette: vec![chest.try_into_api().unwrap()],
                filters: Vec::new(),
                runs: vec![BlockRun {
                    x: POS.0,
                    y: POS.1,
                    z: POS.2,
                    height: 1,
                    surface: None,
                    block: 0,
                    only: None,
                }],
            };
            game.world.block_entities.set_mod_containers(
                [(
                    original.0,
                    Arc::new(ModContainer {
                        slots: 1,
                        ..Default::default()
                    }),
                )]
                .into_iter()
                .collect(),
            );
            let original_stack = ItemStack::new("minecraft:diamond", 1);
            let chest_stack = ItemStack::new("minecraft:iron_ingot", 1);
            let BlockEntity::Mod(row) = game.world.block_entities.get_mut(&POS).unwrap() else {
                panic!("expected mod container");
            };
            row.items = vec![Some(original_stack.clone())];
            game.world.block_log = Some(Vec::new());

            let seal = game.transaction_open();
            if strict_first {
                assert_eq!(game.edit_blocks("minecraft:overworld", &edit), Ok(1));
                game.world.block_entities.get_mut(&POS).unwrap().slots_mut()[0] =
                    Some(chest_stack.clone());
            }
            assert_eq!(
                game.level_set_block(
                    level,
                    AT,
                    glass.try_into_api().unwrap(),
                    BlockUpdateFlags::KNOWN_SHAPE
                ),
                Ok(true)
            );
            assert_eq!(game.edit_blocks("minecraft:overworld", &edit), Ok(1));
            let last_chest = game.block_entity(level, AT).unwrap();
            assert_ne!(last_chest, subject);
            game.transaction_close(&seal, true);

            assert_eq!(game.world.block(POS.0, POS.1, POS.2), chest);
            assert_eq!(game.block_entity(level, AT), Some(last_chest));
            assert!(
                game.world
                    .block_entities
                    .get(&POS)
                    .unwrap()
                    .slots()
                    .iter()
                    .all(Option::is_none)
            );
            assert!(game.world.bulk_log.contains(&CHUNK));
            let [drop] = game.dropped.as_slice() else {
                panic!("one ordinary spill, none from strict")
            };
            assert_eq!(
                drop.stack,
                if strict_first {
                    chest_stack
                } else {
                    original_stack
                }
            );
        });
    }
}

#[test]
#[ignore = "daily"]
fn replacement_and_item_stack_journals_close_in_their_own_dimensions() {
    let scratch = tempfile::tempdir().unwrap();
    with_game(|game| {
        game.world_root = scratch.path().into();
        let overworld = place_subject(game, KIND);
        game.enter_level(Dim::Nether).unwrap();
        let nether = place_subject(game, KIND);
        let original = game.world.block(POS.0, POS.1, POS.2);
        let replacement = game
            .pack
            .assets
            .registry
            .default_state("minecraft:glass")
            .unwrap();
        let state = replacement.try_into_api().unwrap();
        // Keep distinct edited rows and stacks at exactly the same coordinates.
        for dim in [Dim::Nether, Dim::Overworld] {
            game.enter_level(dim).unwrap();
            game.world.block_entities.set_mod_containers(
                [(
                    original.0,
                    Arc::new(ModContainer {
                        slots: 2,
                        ..Default::default()
                    }),
                )]
                .into_iter()
                .collect(),
            );
            let BlockEntity::Mod(row) = game.world.block_entities.get_mut(&POS).unwrap() else {
                panic!("expected a mod container");
            };
            let item = if dim == Dim::Nether {
                "minecraft:gold_ingot"
            } else {
                "minecraft:iron_ingot"
            };
            row.items = vec![
                Some(ItemStack::new("minecraft:diamond", 1)),
                Some(ItemStack::new(item, 1)),
            ];
            game.spawn_item(ItemStack::new(item, 8), [1.0, 64.0, 1.0], [0.0; 3], 0);
        }
        let replace = |game: &mut Game<'_>, subject| {
            let item = game
                .item_entity(game.entity(game.dropped[0].id).unwrap())
                .unwrap();
            let mut access = access(game, subject);
            let mut tick = BlockEntityTickScope::new(&mut access);
            let (level, pos) = tick
                .with_view(|mut view| {
                    assert_eq!(view.set_item_slot(0, None), Ok(true));
                    (view.level(), view.position())
                })
                .unwrap();
            let host = tick.host();
            let stack = item
                .with_stack(host, |s| s.project())
                .flatten()
                .unwrap()
                .shrink(1)
                .unwrap();
            assert_eq!(item.set_stack(host, stack), Ok(true));
            assert_eq!(
                level.set_block(host, pos, state, BlockUpdateFlags::KNOWN_SHAPE),
                Ok(true)
            );
        };
        let outer = game.transaction_open();
        replace(game, overworld);
        game.enter_level(Dim::Nether).unwrap();
        let aborted = game.transaction_open();
        replace(game, nether);
        game.transaction_close(&aborted, false);
        let level = game.level("minecraft:the_nether").unwrap();
        assert_eq!(game.block_entity(level, AT), Some(nether));
        assert_eq!(game.world.block(POS.0, POS.1, POS.2), original);
        assert_eq!(game.dropped[0].stack.count, 8);
        assert_eq!(game.dropped.len(), 1);
        let committed = game.transaction_open();
        replace(game, nether);
        game.transaction_close(&committed, true);
        game.enter_level(Dim::Overworld).unwrap();
        game.transaction_close(&outer, false);
        let level = game.level("minecraft:overworld").unwrap();
        assert_eq!(game.block_entity(level, AT), Some(overworld));
        assert_eq!(game.world.block(POS.0, POS.1, POS.2), original);
        assert_eq!(game.dropped[0].stack.count, 8);
        assert_eq!(game.dropped.len(), 1);
        game.enter_level(Dim::Nether).unwrap();
        assert_eq!(game.world.block(POS.0, POS.1, POS.2), replacement);
        assert_eq!(
            game.block_entity(game.level("minecraft:the_nether").unwrap(), AT),
            None
        );
        assert_eq!(game.dropped[0].stack.count, 7);
        assert_eq!(
            game.dropped[1].stack,
            ItemStack::new("minecraft:gold_ingot", 1)
        );
        assert_eq!(game.dropped.len(), 2);
        game.enter_level(Dim::Overworld).unwrap();
    });
}

struct Tick;
#[mod_api::sunrise_part]
impl BlockPart for Tick {}
#[mod_api::sunrise_part]
impl BlockEntityPart for Tick {
    fn block_entity_tick(&self, event: &mut BlockEntityTickEvent<'_>) {
        event
            .subject
            .with_view(|mut view| assert!(view.set_data_slot(0, 1)))
            .unwrap();
    }
}

struct Effects(Arc<AtomicU32>);
#[mod_api::sunrise_part]
impl BlockPart for Effects {
    fn contents_changed(&self, _: &mut dyn ServerHost, event: &mut mod_api::ContentsChangedEvent) {
        if event.pos == mod_api::BlockPos::new(POS.0, POS.1, POS.2) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
}
#[mod_api::__subscriptions]
impl Hooks for Effects {
    fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        let kind = r.block_entity_type("subscribed").build();
        r.block_entity_part(&kind, Tick).unwrap();
        r.block_entity_type("unsubscribed").build();
        let stone = r.block_key("minecraft:stone");
        r.block_part(stone, Effects(self.0.clone()));
    }
}

fn with_ticker(test: impl FnOnce(&mut Game<'_>, &AtomicU32)) {
    let notices = Arc::new(AtomicU32::new(0));
    let mods: mod_api::Mods = vec![ModEntry::new(
        "view_tick",
        Box::new(Effects(notices.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods.clone(), || {
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_api::__private::mark_game_thread();
        let base = crate::game::native_entity_refs::tests::fixture_root();
        let root = crate::game::mods::content_root(&base, &Default::default()).unwrap();
        with_game_at(root.prepared().to_owned(), |game| test(game, &notices));
    });
}

#[test]
#[ignore = "daily"]
fn ticker_and_handle_setters_dirty_without_explicit_notices() {
    const NAME: &str = "game::native_hooks::block_entity_tick::tests::ticker_and_handle_setters_dirty_without_explicit_notices";
    if run_fresh_cases(NAME, "SUNRISE_RESIDENT_VIEW_TICK", &["run"]) {
        return;
    }
    with_ticker(|game, notices| {
        let subject = place_subject(game, KIND);
        game.world.block_log = Some(Vec::new());
        game.tick_mod_block_entity(POS);
        assert_eq!(subject.data_slots(game).unwrap(), [1]);
        assert!(game.world.modified.contains(&CHUNK));
        assert!(game.world.block_log.as_ref().unwrap().is_empty());
        assert_eq!(notices.load(Ordering::Relaxed), 0);
        game.world.modified.clear();
        assert!(subject.set_data_slot(game, 0, 2).unwrap());
        assert_eq!(subject.data_slots(game).unwrap(), [2]);
        assert!(game.world.modified.contains(&CHUNK));
        assert!(game.world.block_log.as_ref().unwrap().is_empty());
        assert_eq!(notices.load(Ordering::Relaxed), 0);
        subject.set_changed(game).unwrap();
        assert_eq!(
            notices.load(Ordering::Relaxed),
            1,
            "explicit notice is wired"
        );
    });
}

#[test]
#[ignore = "daily"]
fn a_registered_type_without_a_tick_part_is_not_woken_or_changed() {
    const NAME: &str = "game::native_hooks::block_entity_tick::tests::a_registered_type_without_a_tick_part_is_not_woken_or_changed";
    if run_fresh_cases(NAME, "SUNRISE_RESIDENT_VIEW_NO_TICK", &["run"]) {
        return;
    }
    with_ticker(|game, notices| {
        let subject = place_subject(game, UNSUBSCRIBED);
        game.world.block_entities.sleep(POS);
        game.tick_mod_block_entity(POS);
        assert_eq!(subject.data_slots(game).unwrap(), [23]);
        assert!(game.world.block_entities.is_asleep(&POS));
        assert!(game.world.modified.is_empty());
        assert_eq!(notices.load(Ordering::Relaxed), 0);
    });
}
