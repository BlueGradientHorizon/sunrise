#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::AT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::Baby;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::BabyFn;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::DiscardParentOnPigJoin;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::IGNORE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::NEXT_INT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::NetherBiomeOnJoin;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::SEED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::Split;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::SplitFn;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::YAWS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::animals_bred;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::assert_vanilla_children;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::breed_pair;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::find;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::near;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::spawned;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::spec;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::with_baby;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::with_split;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::with_splits;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pending_birth_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::{
    run_fresh_cases, with_game,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    __private::{HookPanicReport, take_hook_panics},
    BabyEntitySpawnEvent, BlockState, BlockUpdateFlags, Flow, Hooks, HostBlockStates,
    MobSplitEvent, PendingMobSpec, ServerHost, UnspawnedMobError,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, AtomicUsize, Ordering},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::chunk::Chunk;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::random::LegacyRandom;

#[test]
#[ignore = "daily"]
fn a_panicking_baby_listener_leaves_no_reserved_state_behind() {
    use crate::game::native_entity_refs::tests::bind_local_player;
    use sunrise_client::{
        advancements::{Advancement, Criterion, Rewards, Tree},
        motion::PlayerMotion,
    };

    if run_fresh_cases(
        "game::server_side::creatures_server::pending_birth_tests::a_panicking_baby_listener_leaves_no_reserved_state_behind",
        "SUNRISE_BABY_PANIC_CHILD",
        &["breed"],
    ) {
        return;
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let record = calls.clone();
    with_baby(
        move |_, event| {
            assert_eq!(event.player, Some(7));
            record.fetch_add(1, Ordering::SeqCst);
            panic!("a listener fails");
        },
        |game| {
            game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
            bind_local_player(game, 700, 7);
            const ADVANCEMENT: &str = "test:breed";
            const TRIGGER: &str = "minecraft:bred_animals";
            let mut tree = Tree::default();
            tree.list.push(Advancement {
                id: ADVANCEMENT.into(),
                parent: None,
                display: None,
                criteria: vec![Criterion {
                    name: "bred".into(),
                    trigger: TRIGGER.into(),
                    conditions: serde_json::json!({}).into(),
                }],
                requirements: vec![vec!["bred".into()]],
                rewards: Rewards::default(),
            });
            tree.by_id.insert(ADVANCEMENT.into(), 0);
            tree.by_trigger.insert(TRIGGER.into(), vec![(0, 0)]);
            game.adv.tree = Arc::new(tree);
            game.adv.loaded = true;
            let level_random = game.world.random.state();
            let (uuids, attachments) = (game.persist.uuids.len(), game.entity_attachments.len());
            let (parent, partner, child) = breed_pair_credited(game, "minecraft:cow", true);
            assert_eq!(child, None);
            assert_eq!(
                game.mobs.iter().map(|m| m.id).collect::<Vec<_>>(),
                [parent, partner]
            );
            assert_eq!(game.persist.uuids.len(), uuids, "the baby's UUID is gone");
            assert_eq!(game.entity_attachments.len(), attachments);
            assert_eq!(game.world.random.state(), level_random, "no level yaw draw");
            // Cow.getBreedOffspring takes one nextBoolean from Java Random(0); the
            // second draw is still next, since Animal's finalization did not draw XP.
            assert_eq!(find(game, parent).random.next_int(), -723_955_400);
            for id in [parent, partner] {
                let mob = find(game, id);
                assert_eq!(
                    mob.extra.age, 6000,
                    "NeoForge Animal.spawnChildFromBreeding"
                );
                assert_eq!(mob.extra.in_love, 0);
                assert!(!game.side.love_cause.contains_key(&id));
            }
            assert!(game.orbs.is_empty());
            assert_eq!(animals_bred(game), 0);
            assert!(!game.adv.progress.has(ADVANCEMENT, "bred"));
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(
                take_hook_panics(),
                [HookPanicReport {
                    mod_id: "baby",
                    hook: "baby_entity_spawn"
                }]
            );

            let (_, _, child) = breed_pair_credited(game, "minecraft:cow", true);
            let baby = find(game, child.expect("the next baby joined"));
            assert!(baby.extra.age < 0);
            assert_eq!(baby.yaw, 0.0);
            assert!(game.persist.uuids.contains_key(&baby.id));
            assert_eq!(game.orbs.len(), 1);
            assert_eq!(animals_bred(game), 1);
            assert!(game.adv.progress.has(ADVANCEMENT, "bred"));
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert!(take_hook_panics().is_empty());
        },
    );
}

#[test]
#[ignore = "daily"]
fn a_panicking_listener_spawns_nothing_and_leaves_the_arena_usable() {
    if run_fresh_cases(
        "game::server_side::creatures_server::pending_birth_tests::a_panicking_listener_spawns_nothing_and_leaves_the_arena_usable",
        "SUNRISE_SPLIT_PANIC_CHILD",
        &["split"],
    ) {
        return;
    }
    const WRITE_AT: [i32; 3] = [4, 64, 4];
    let calls = Arc::new(AtomicUsize::new(0));
    let record = calls.clone();
    with_split(
        move |host, event| {
            let child = event
                .edits
                .child(event.edits.children().unwrap(), 0)
                .unwrap();
            event.edits.set_yaw(child, 1.0).unwrap();
            let level = host.level(event.dimension).unwrap();
            let key = host.block_key("minecraft:stone").unwrap();
            let stone = BlockState::of(&host.block_states(), key, &[]).unwrap();
            assert_eq!(
                level.set_block(host, WRITE_AT, stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(level.block_state(host, WRITE_AT), Some(stone));
            // A contained failure before the edit and write above must not pass this test.
            record.fetch_add(1, Ordering::SeqCst);
            panic!("a listener fails before commit");
        },
        |game| {
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let [x, y, z] = WRITE_AT;
            let old_block = game
                .pack
                .assets
                .registry
                .default_state("minecraft:dirt")
                .unwrap();
            game.world.set(x, y, z, old_block).unwrap();
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            let next_id = game.side.next_entity;
            game.split_cube_children(&mut dying);
            assert!(game.mobs.is_empty(), "no child joined");
            assert_eq!(
                game.side.next_entity, next_id,
                "no entity number was issued"
            );
            assert_eq!(
                game.world.block(x, y, z),
                old_block,
                "tentative block write survived"
            );
            assert_eq!(dying.random.next_int(), NEXT_INT);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(
                take_hook_panics(),
                [HookPanicReport {
                    mod_id: "split",
                    hook: "mob_split"
                }]
            );

            // The same install still takes the arena path, but skips the retired listener.
            let mut again = cube(game, "minecraft:slime", 4, SEED);
            let first = game.side.next_entity + 1;
            game.split_cube_children(&mut again);
            assert_eq!(game.mobs.len(), YAWS.len());
            for (i, (child, yaw)) in game.mobs.iter().zip(YAWS).enumerate() {
                assert_eq!(child.id, first + i as u32);
                assert_eq!(child.yaw, yaw);
            }
            assert_eq!(again.random.next_int(), NEXT_INT);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert!(take_hook_panics().is_empty());
        },
    );
}

/// What a listener's host write does to a parent, stood in for by a closure the production
/// dispatch runs where the listeners run (the host methods that move a mob are not reachable
/// from a hook's `ServerHost` in the fixture world).
#[test]
#[ignore = "daily"]
fn a_parent_changed_during_dispatch_refuses_the_baby_and_nothing_is_rebuilt() {
    if run_fresh_cases(
        "game::server_side::creatures_server::pending_birth_tests::a_parent_changed_during_dispatch_refuses_the_baby_and_nothing_is_rebuilt",
        "SUNRISE_BIRTH_STALE_CHILD",
        &["stale"],
    ) {
        return;
    }
    with_game(|game| {
        let kind = game.res.mob_types().get("minecraft:cow").unwrap().clone();
        let base = game.side.next_entity;
        game.side.next_entity += 3;
        for (i, x) in [0.0, 1.0].into_iter().enumerate() {
            game.mobs.push(Mob::new(
                base + 1 + i as u32,
                kind.clone(),
                Vec3::new(x, 64.0, 0.0),
                0.0,
            ));
        }
        game.native_joins_skip();
        let run = |game: &mut crate::game::ServerGame<'_>, change: bool| {
            let mut baby = Mob::new(base + 3, kind.clone(), Vec3::new(0.5, 64.0, 0.0), 0.0);
            sunrise_client::mobs::finalize::set_baby(&mut baby, true);
            game.dispatch_pending_children(
                mod_api::HookId::BabyEntitySpawn,
                true,
                vec![baby],
                &[base + 1, base + 2],
                move |game, _, _| {
                    if change {
                        let parent = game.mobs.iter_mut().find(|m| m.id == base + 1).unwrap();
                        parent.extra.in_love = 600;
                    }
                },
                |_, children| children.len(),
            )
        };
        // Unchanged parents: the baby is accepted.
        assert_eq!(run(game, false), Some(1));
        // The id is still live but the parent is not what the baby was built from: refused,
        // and the dispatch never rebuilds or draws again.
        let draws = game
            .mobs
            .iter()
            .map(|m| m.random.state())
            .collect::<Vec<_>>();
        assert_eq!(run(game, true), None);
        assert_eq!(
            game.mobs
                .iter()
                .map(|m| m.random.state())
                .collect::<Vec<_>>(),
            draws
        );
    });
}

/// [`breed_pair`] with the host player credited for feeding the parents into love (what the
/// goal records in `love_cause`), when `credited`.
pub(in crate::game) fn breed_pair_credited(
    game: &mut crate::game::ServerGame<'_>,
    kind: &str,
    credited: bool,
) -> (u32, u32, Option<u32>) {
    let kind = game.res.mob_types().get(kind).unwrap().clone();
    let base = game.side.next_entity;
    game.side.next_entity += 2;
    game.mobs.push(Mob::new(
        base + 1,
        kind.clone(),
        Vec3::new(0.0, 64.0, 0.0),
        0.0,
    ));
    game.mobs
        .push(Mob::new(base + 2, kind, Vec3::new(1.0, 64.0, 0.0), 0.0));
    game.native_joins_skip();
    if credited {
        game.side.love_cause.insert(base + 1, None);
    }
    let parent = game.mobs.iter_mut().find(|m| m.id == base + 1).unwrap();
    parent.random = LegacyRandom::new(0);
    parent.extra.age = 6000;
    let partner = game.mobs.iter_mut().find(|m| m.id == base + 2).unwrap();
    partner.extra.in_love = 600;
    let child = game.breed(
        base + 1,
        base + 2,
        None,
        None,
        None,
        sunrise_client::mobs::BreedExperience::Deferred,
    );
    (base + 1, base + 2, child)
}

#[test]
#[ignore = "daily"]
fn cancelled_split_spawns_nothing_but_keeps_the_construction_draws() {
    with_split(
        |_, _| Flow::Cancel,
        |game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            let next_id = game.side.next_entity;
            game.split_cube_children(&mut dying);
            assert!(game.mobs.is_empty(), "a cancelled split spawns no child");
            assert_eq!(
                game.side.next_entity, next_id,
                "and issues no entity number"
            );
            // The count and the three yaw draws were spent before the callback.
            assert_eq!(dying.random.next_int(), NEXT_INT);
        },
    );
}

#[test]
#[ignore = "daily"]
fn credit_goes_to_the_player_only_when_the_baby_is_accepted() {
    with_baby(
        |_, _| Flow::Continue,
        |game| {
            let (parent, partner, child) = breed_pair_credited(game, "minecraft:cow", true);
            assert!(child.is_some());
            assert_eq!(animals_bred(game), 1, "an accepted baby is credited once");
            assert!(!game.side.love_cause.contains_key(&parent));
            assert!(!game.side.love_cause.contains_key(&partner));
        },
    );
    with_baby(
        |_, _| Flow::Cancel,
        |game| {
            let (parent, partner, child) = breed_pair_credited(game, "minecraft:cow", true);
            assert_eq!(child, None);
            assert_eq!(animals_bred(game), 0, "a vetoed baby earns no statistic");
            assert!(
                !game.side.love_cause.contains_key(&parent),
                "the credit is spent"
            );
            assert!(!game.side.love_cause.contains_key(&partner));
        },
    );
}

/// A dying cube mob of `kind` and `size` whose own random is `new Random(seed)`.
pub(in crate::game) fn cube(
    game: &mut crate::game::ServerGame<'_>,
    kind: &str,
    size: i32,
    seed: i64,
) -> Mob {
    game.side.next_entity += 1;
    let kind = game.res.mob_types().get(kind).unwrap().clone();
    let mut mob = Mob::new(
        game.side.next_entity,
        kind,
        Vec3::new(AT[0], AT[1], AT[2]),
        0.0,
    );
    if mob.species() == Species::SulfurCube {
        sunrise_client::mobs::sulfur_cube::set_size(&mut mob, size, true);
    } else {
        sunrise_client::mobs::cube::set_size(&mut mob, size);
    }
    mob.random = LegacyRandom::new(seed);
    mob
}

#[test]
#[ignore = "daily"]
fn edit_remove_append_commits_a_c_d_once_and_b_goes_stale() {
    let stale = Arc::new(Mutex::new(None));
    let record = stale.clone();
    with_split(
        move |host, event| {
            let (edits, children) = (event.edits, event.edits.children().unwrap());
            let a = edits.child(children, 0).unwrap();
            let b = edits.child(children, 1).unwrap();
            edits.set_yaw(a, 90.0).unwrap();
            edits.remove(children, 1).unwrap();
            *record.lock().unwrap() = Some(b.kind(&*host));
            let d = edits
                .construct(
                    children,
                    spec(
                        "minecraft:slime",
                        [3.0, 70.0, 4.0],
                        12.0,
                        Some(2),
                        event.dimension,
                    ),
                )
                .unwrap();
            edits.append(children, d).unwrap();
            assert_eq!(edits.len(children), Ok(3));
            Flow::Continue
        },
        |game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            let first = game.side.next_entity + 1;
            game.split_cube_children(&mut dying);
            let kids = spawned(game);
            // [A, C, D]: A with yaw 90, C the vanilla third child, D appended.
            assert_eq!(kids.len(), 3);
            assert_eq!(kids[0].yaw, 90.0);
            assert_eq!(kids[0].id, first);
            assert!(near(kids[1].position.x, AT[0] - 0.52));
            assert!(near(kids[1].position.z, AT[2] + 0.52));
            assert_eq!(kids[1].yaw, YAWS[2]);
            assert_eq!(kids[1].id, first + 1);
            assert_eq!(kids[2].position.x, 3.0);
            assert_eq!(kids[2].position.y, 70.0);
            assert_eq!(kids[2].yaw, 12.0);
            assert_eq!(kids[2].id, first + 2);
            assert_eq!(kids[2].extra.size, 2);
        },
    );
    assert_eq!(
        stale.lock().unwrap().clone(),
        Some(Err(UnspawnedMobError::Expired)),
        "the removed child's borrow is stale"
    );
}

#[test]
#[ignore = "daily"]
fn no_listener_split_is_exactly_vanilla() {
    let mods: mod_api::Mods = Vec::new().into();
    mod_platform::testing::with_mods(mods, || {
        with_game(|game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            let first = game.side.next_entity + 1;
            game.split_cube_children(&mut dying);
            assert_vanilla_children(&spawned(game), "minecraft:slime", first);
            assert_eq!(dying.random.next_int(), NEXT_INT);
        });
    });
}

#[test]
#[ignore = "daily"]
fn size_four_slime_and_magma_split_three_children_and_draw_the_vanilla_yaws() {
    let _ = IGNORE;
    for kind in ["minecraft:slime", "minecraft:magma_cube"] {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = seen.clone();
        with_split(
            move |_, event| {
                let len = event.edits.len(event.edits.children().unwrap()).unwrap();
                record.lock().unwrap().push((
                    event.dimension.to_owned(),
                    event.kind.to_owned(),
                    event.size,
                    event.child_size,
                    len,
                ));
                Flow::Continue
            },
            |game| {
                let mut dying = cube(game, kind, 4, SEED);
                let first = game.side.next_entity + 1;
                game.split_cube_children(&mut dying);
                assert_vanilla_children(&spawned(game), kind, first);
                assert_eq!(dying.random.next_int(), NEXT_INT);
            },
        );
        assert_eq!(
            *seen.lock().unwrap(),
            [("minecraft:overworld".to_owned(), kind.to_owned(), 4, 2, 3)]
        );
    }
}
