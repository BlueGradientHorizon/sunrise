use super::*;
use crate::game::native_entity_refs::tests::{run_fresh_cases, with_game, with_game_at};
use mod_api::{
    BlockEntityPart, BlockEntityTickEvent, BlockPart, Hooks, HostTransaction, ModEntry, Registrar,
};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use sunrise_client::{chunk::Chunk, containers::ModBlockEntityFields};

const POS: Pos = (1, 64, 1);
const CHUNK: (i32, i32) = (0, 0);
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

fn access<'a, 'g>(
    game: &'a mut Game<'g>,
    subject: BlockEntityRef,
) -> ResidentBlockEntityAccess<'a, 'g> {
    let state = game.world.block(POS.0, POS.1, POS.2);
    ResidentBlockEntityAccess::for_handle(game, subject, state)
}

#[test]
#[ignore = "daily"]
fn retaking_the_view_checks_the_instance_and_state() {
    with_game(|game| {
        let subject = place_subject(game, KIND);
        let original_state = game.world.block(POS.0, POS.1, POS.2);
        let mut access = ResidentBlockEntityAccess::for_handle(game, subject, original_state);
        let mut scope = BlockEntityScope::new(&mut access);
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
        assert_eq!(BlockEntityScope::new(&mut access).with_view(|_| ()), None);
        access
            .game
            .world
            .set(pos[0], pos[1], pos[2], original_state)
            .unwrap();
        assert_eq!(
            BlockEntityScope::new(&mut access).with_view(|_| ()),
            Some(())
        );
        access.game.world.block_entities.remove(&POS);
        assert_eq!(BlockEntityScope::new(&mut access).with_view(|_| ()), None);
        access.game.world.block_entities.insert(POS, row(KIND, 99));
        assert_eq!(BlockEntityScope::new(&mut access).with_view(|_| ()), None);
    });
}

#[test]
#[ignore = "daily"]
fn read_only_and_noop_loans_do_not_journal_or_dirty() {
    with_game(|game| {
        let subject = place_subject(game, KIND);
        let result: Result<(), ()> = game.transaction(|game, _| {
            let mut access = access(game, subject);
            BlockEntityScope::new(&mut access)
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
        let mut scope = BlockEntityScope::new(&mut access);
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

#[test]
#[ignore = "daily"]
fn blockless_scope_returns_none_for_view_and_keeps_host_and_transaction_working() {
    with_game(|game| {
        let subject = place_subject(game, KIND);
        let mut access = ResidentBlockEntityAccess::blockless(game);
        let mut scope = BlockEntityScope::new(&mut access);
        assert_eq!(scope.with_view(|_| 1u32), None);
        let aborted: Result<(), ()> = scope.transaction(|scope, _| {
            assert_eq!(scope.with_view(|_| 1u32), None);
            assert!(subject.set_data_slot(scope.host(), 0, 50).unwrap());
            Err(())
        });
        assert_eq!(aborted, Err(()));
        assert_eq!(subject.data_slots(scope.host()).unwrap(), [23]);
        let committed: Result<(), ()> = scope.transaction(|scope, _| {
            assert!(subject.set_data_slot(scope.host(), 0, 55).unwrap());
            Ok(())
        });
        assert_eq!(committed, Ok(()));
        assert_eq!(subject.data_slots(scope.host()).unwrap(), [55]);
        assert!(!access.items_changed());
    });
}

#[test]
#[ignore = "daily"]
fn items_changed_tracks_committed_slot_writes_and_ignores_aborted_nested_transactions() {
    with_game(|game| {
        let subject = place_subject(game, KIND);
        let (handle, state) = game.resident_block_entity_target(POS).unwrap();
        assert_eq!(handle, subject);
        game.world.block_entities.set_mod_containers(
            [(
                state.0,
                Arc::new(sunrise_client::containers::ModContainer {
                    slots: 2,
                    ..Default::default()
                }),
            )]
            .into_iter()
            .collect(),
        );
        let stone = sunrise_client::items::ItemStack::new("minecraft:stone", 1)
            .try_into_api()
            .unwrap();
        let mut access = ResidentBlockEntityAccess::for_handle(game, handle, state);
        {
            let mut scope = BlockEntityScope::new(&mut access);
            assert_eq!(
                scope
                    .with_view(|mut view| view.set_item_slot(0, None))
                    .unwrap(),
                Ok(false)
            );
        }
        assert!(!access.items_changed());

        {
            let mut scope = BlockEntityScope::new(&mut access);
            let aborted: Result<(), ()> = scope.transaction(|scope, _| {
                assert_eq!(
                    scope
                        .with_view(|mut view| view.set_item_slot(0, Some(stone.clone())))
                        .unwrap(),
                    Ok(true)
                );
                Err(())
            });
            assert_eq!(aborted, Err(()));
        }
        assert!(!access.items_changed());

        {
            let mut scope = BlockEntityScope::new(&mut access);
            let committed: Result<(), ()> = scope.transaction(|scope, _| {
                assert_eq!(
                    scope
                        .with_view(|mut view| view.set_item_slot(0, Some(stone.clone())))
                        .unwrap(),
                    Ok(true)
                );
                Ok(())
            });
            assert_eq!(committed, Ok(()));
            assert_eq!(
                scope
                    .with_view(|view| view.item_slot(0).map(|s| s.count()))
                    .unwrap(),
                Some(1)
            );
        }
        assert!(access.items_changed());
    });
}
