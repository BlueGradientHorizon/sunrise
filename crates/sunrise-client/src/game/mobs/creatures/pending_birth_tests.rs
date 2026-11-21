//! Real-route fixtures for the pending-children hooks: `mob_split` through the slime/magma/sulfur
//! split and `baby_entity_spawn` through breeding. Each test is named for its row of the
//! acceptance matrix in docs/design/mod-api/pending-children.md. The expected values are
//! written out: the seeds' draws come from Java's `java.util.Random` (`new Random(2)` gives
//! `nextInt(3) = 1`, so three children; then three `nextFloat() * 360.0F` yaws;
//! see `YAWS`), and the offsets are vanilla's `setUpSplitCube` arithmetic spelled out in the test.
//!
//! They need prepared resources (`SUNRISE_TEST_RESOURCES`) and run with a native test mod
//! installed for the test only.

use super::*;
use crate::game::native_entity_refs::tests::with_game;
use mod_api::{
    AttachmentType, BabyEntitySpawnEvent, EntityConstructingEvent, EntityJoinLevelEvent, Flow,
    Holder, Hooks, HostSide, MobSplitEvent, PendingMobSpec, ServerHost, UnspawnedMob,
    UnspawnedMobError,
};
use std::sync::{Arc, Mutex};
use sunrise_core::random::LegacyRandom;

const IGNORE: &str = "needs prepared resources in SUNRISE_TEST_RESOURCES";

/// `new Random(2)`: `2 + nextInt(3)` is 3, then `nextFloat() * 360.0F` three times, then the
/// next `nextInt()`. The same three draws with no listener, a cancel or a listener.
pub(super) const SEED: i64 = 2;
pub(super) const YAWS: [f32; 3] = [105.61554, 324.52115, 1.4961791];
const NEXT_INT: i32 = 2_133_836_778;

type SplitFn = Box<dyn Fn(&mut dyn ServerHost, &mut MobSplitEvent<'_>) -> Flow + Send + Sync>;
struct Split(SplitFn);
#[mod_api::__subscriptions]
impl Hooks for Split {
    fn mob_split(&self, host: &mut dyn ServerHost, event: &mut MobSplitEvent<'_>) -> Flow {
        (self.0)(host, event)
    }
}

type BabyFn = Box<dyn Fn(&mut dyn ServerHost, &mut BabyEntitySpawnEvent<'_>) -> Flow + Send + Sync>;
struct Baby(BabyFn);
#[mod_api::__subscriptions]
impl Hooks for Baby {
    fn baby_entity_spawn(
        &self,
        host: &mut dyn ServerHost,
        event: &mut BabyEntitySpawnEvent<'_>,
    ) -> Flow {
        (self.0)(host, event)
    }
}

fn with_split(
    listener: impl Fn(&mut dyn ServerHost, &mut MobSplitEvent<'_>) -> Flow + Send + Sync + 'static,
    run: impl FnOnce(&mut Game<'_>),
) {
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "split",
        Box::new(Split(Box::new(listener))),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || with_game(run));
}

fn with_baby(
    listener: impl Fn(&mut dyn ServerHost, &mut BabyEntitySpawnEvent<'_>) -> Flow
    + Send
    + Sync
    + 'static,
    run: impl FnOnce(&mut Game<'_>),
) {
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "baby",
        Box::new(Baby(Box::new(listener))),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || with_game(run));
}

const AT: [f64; 3] = [10.0, 64.0, 20.0];

/// A dying cube mob of `kind` and `size` whose own random is `new Random(seed)`.
pub(super) fn cube(game: &mut Game<'_>, kind: &str, size: i32, seed: i64) -> Mob {
    game.next_entity += 1;
    let kind = game.res.mob_types().get(kind).unwrap().clone();
    let mut mob = Mob::new(game.next_entity, kind, Vec3::new(AT[0], AT[1], AT[2]), 0.0);
    if mob.species() == Species::SulfurCube {
        sunrise_client::mobs::sulfur_cube::set_size(&mut mob, size, true);
    } else {
        sunrise_client::mobs::cube::set_size(&mut mob, size);
    }
    mob.random = LegacyRandom::new(seed);
    mob
}

pub(super) fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-4
}

/// The mobs the split added, in order (the fixture world starts with none).
fn spawned(game: &Game<'_>) -> Vec<Mob> {
    game.mobs.to_vec()
}

fn assert_vanilla_children(children: &[Mob], kind: &str, first_id: u32) {
    assert_eq!(children.len(), 3);
    // `setUpSplitCube`: a half-width offset spread over a 2x2 grid by index, 0.5 up. A size-4
    // cube is 2.08 wide (0.52 per size), so the offset is 1.04.
    let offset = 1.04;
    for (i, child) in children.iter().enumerate() {
        assert_eq!(child.kind.id, kind);
        assert_eq!(child.id, first_id + i as u32, "ids are issued in order");
        assert_eq!(child.extra.size, 2);
        assert!(near(
            child.position.x,
            AT[0] + ((i % 2) as f64 - 0.5) * offset
        ));
        assert!(near(child.position.y, AT[1] + 0.5));
        assert!(near(
            child.position.z,
            AT[2] + ((i / 2) as f64 - 0.5) * offset
        ));
        assert_eq!(child.yaw, YAWS[i]);
    }
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
                let len = event.children.len();
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
                let first = game.next_entity + 1;
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

#[test]
#[ignore = "daily"]
fn cancelled_split_spawns_nothing_but_keeps_the_construction_draws() {
    with_split(
        |_, _| Flow::Cancel,
        |game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            let next_id = game.next_entity;
            game.split_cube_children(&mut dying);
            assert!(game.mobs.is_empty(), "a cancelled split spawns no child");
            assert_eq!(game.next_entity, next_id, "and issues no entity number");
            // The count and the three yaw draws were spent before the callback.
            assert_eq!(dying.random.next_int(), NEXT_INT);
        },
    );
}

#[test]
#[ignore = "daily"]
fn no_listener_split_is_exactly_vanilla() {
    let mods: mod_api::Mods = Vec::new().into();
    mod_platform::testing::with_mods(mods, || {
        with_game(|game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            let first = game.next_entity + 1;
            game.split_cube_children(&mut dying);
            assert_vanilla_children(&spawned(game), "minecraft:slime", first);
            assert_eq!(dying.random.next_int(), NEXT_INT);
        });
    });
}

#[test]
#[ignore = "daily"]
fn sulfur_cube_splits_two_babies_unless_primed_and_size_one_asks_nobody() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let push_when_empty = Arc::new(Mutex::new(false));
    let record = seen.clone();
    let do_push = push_when_empty.clone();
    with_split(
        move |_, event| {
            let empty = event.children.is_empty();
            record.lock().unwrap().push((
                event.kind.to_owned(),
                event.size,
                event.child_size,
                event.children.len(),
            ));
            if empty && *do_push.lock().unwrap() {
                let extra = event
                    .children
                    .construct(spec(
                        "minecraft:slime",
                        [10.0, 64.5, 20.0],
                        15.0,
                        Some(2),
                        event.dimension,
                    ))
                    .unwrap();
                event.children.push(extra).unwrap();
            }
            Flow::Continue
        },
        |game| {
            // Unprimed, size 4: two babies of size 2.
            let dying = cube(game, "minecraft:sulfur_cube", 4, 0);
            game.split_dead_cube(dying);
            let kids = spawned(game);
            assert_eq!(kids.len(), 2);
            assert!(
                kids.iter().all(|k| k.is_baby()
                    && k.extra.size == 2
                    && k.kind.id == "minecraft:sulfur_cube")
            );
            // Primed, size 4: NeoForge's AbstractCubeMob.remove still posts MobSplitEvent with
            // an empty list. Leaving it empty spawns nothing.
            game.mobs.clear();
            let mut dying = cube(game, "minecraft:sulfur_cube", 4, 0);
            dying.extra.counter2 = 40;
            assert!(sunrise_client::mobs::sulfur_cube::primed(&dying));
            game.split_dead_cube(dying);
            assert!(game.mobs.is_empty());
            // Populating the initially empty list on a primed sulfur cube spawns the added child.
            *push_when_empty.lock().unwrap() = true;
            let mut dying = cube(game, "minecraft:sulfur_cube", 4, 0);
            dying.extra.counter2 = 40;
            assert!(sunrise_client::mobs::sulfur_cube::primed(&dying));
            game.split_dead_cube(dying);
            assert_eq!(game.mobs.len(), 1);
            assert_eq!(game.mobs[0].kind.id, "minecraft:slime");
            assert_eq!(game.mobs[0].extra.size, 2);
            // A size-1 parent never splits and never asks listeners.
            game.mobs.clear();
            let dying = cube(game, "minecraft:slime", 1, 0);
            game.split_dead_cube(dying);
            assert!(game.mobs.is_empty());
        },
    );
    assert_eq!(
        *seen.lock().unwrap(),
        [
            ("minecraft:sulfur_cube".to_owned(), 4, 2, 2),
            ("minecraft:sulfur_cube".to_owned(), 4, 2, 0),
            ("minecraft:sulfur_cube".to_owned(), 4, 2, 0),
        ]
    );
}

fn spec(kind: &str, at: [f64; 3], yaw: f32, size: Option<u32>, dimension: &str) -> PendingMobSpec {
    let mut spec = PendingMobSpec::new(
        mod_api::Identifier::parse(kind).unwrap(),
        mod_api::Identifier::parse(dimension).unwrap(),
        at,
        yaw,
        false,
    );
    spec.size = size;
    spec
}

#[test]
#[ignore = "daily"]
fn edit_remove_push_commits_a_c_d_once() {
    let second_saw = Arc::new(Mutex::new(None));
    let record = second_saw.clone();
    with_splits(
        vec![
            Box::new(move |_, event| {
                event.children.get_mut(0).unwrap().set_yaw(90.0).unwrap();
                let removed = event.children.remove(1).unwrap();
                assert_eq!(removed.kind().unwrap().to_string(), "minecraft:slime");
                drop(removed);
                let d = event
                    .children
                    .construct(spec(
                        "minecraft:slime",
                        [3.0, 70.0, 4.0],
                        12.0,
                        Some(2),
                        event.dimension,
                    ))
                    .unwrap();
                event.children.push(d).unwrap();
                // A detached child dropped without attaching never joins.
                let _dropped = event
                    .children
                    .construct(spec(
                        "minecraft:slime",
                        [9.0, 70.0, 9.0],
                        0.0,
                        Some(2),
                        event.dimension,
                    ))
                    .unwrap();
                assert_eq!(event.children.len(), 3);
                Flow::Continue
            }),
            Box::new(move |_, event| {
                *record.lock().unwrap() = Some((
                    event.children.len(),
                    event.children.get(2).unwrap().position(),
                ));
                Flow::Continue
            }),
        ],
        |game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            let first = game.next_entity + 1;
            game.split_cube_children(&mut dying);
            let kids = spawned(game);
            // [A, C, D]: A with yaw 90, C the vanilla third child, D pushed.
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
        *second_saw.lock().unwrap(),
        Some((3, Ok([3.0, 70.0, 4.0]))),
        "later listeners see earlier listeners' edits"
    );
}

#[test]
#[ignore = "daily"]
fn replace_c_with_e_commits_a_b_e() {
    let replaced_kind = Arc::new(Mutex::new(None));
    let record = replaced_kind.clone();
    with_split(
        move |_, event| {
            let e = event
                .children
                .construct(spec(
                    "minecraft:magma_cube",
                    [8.0, 65.0, 9.0],
                    33.0,
                    Some(2),
                    event.dimension,
                ))
                .unwrap();
            let prev = event.children.replace(2, e).unwrap();
            *record.lock().unwrap() = Some(prev.kind().unwrap().to_string());
            drop(prev);
            Flow::Continue
        },
        |game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            game.split_cube_children(&mut dying);
            let kids = spawned(game);
            assert_eq!(kids.len(), 3);
            assert_eq!(kids[0].kind.id, "minecraft:slime");
            assert_eq!(kids[1].kind.id, "minecraft:slime");
            assert_eq!(kids[2].kind.id, "minecraft:magma_cube");
            assert_eq!(kids[2].position.x, 8.0);
            assert_eq!(kids[2].yaw, 33.0);
        },
    );
    assert_eq!(
        replaced_kind.lock().unwrap().as_deref(),
        Some("minecraft:slime"),
        "replace returns the previous child"
    );
}

#[test]
#[ignore = "daily"]
fn invalid_input_fails_before_mutation_and_the_valid_list_stands() {
    let errors = Arc::new(Mutex::new(Vec::new()));
    let record = errors.clone();
    with_split(
        move |_, event| {
            let dim = event.dimension;
            let mut out = vec![
                event
                    .children
                    .construct(spec(
                        "minecraft:slime",
                        [f64::NAN, 64.0, 0.0],
                        0.0,
                        Some(2),
                        dim,
                    ))
                    .map(|_| ()),
                event
                    .children
                    .construct(spec(
                        "minecraft:not_a_mob",
                        [0.0, 64.0, 0.0],
                        0.0,
                        None,
                        dim,
                    ))
                    .map(|_| ()),
                event
                    .children
                    .construct(spec(
                        "minecraft:slime",
                        [0.0, 64.0, 0.0],
                        0.0,
                        Some(2),
                        "minecraft:the_end",
                    ))
                    .map(|_| ()),
            ];
            // Out-of-bounds insert and replace refuse without mutating the list.
            let oob_insert = event
                .children
                .construct(spec("minecraft:slime", [0.0, 64.0, 0.0], 0.0, Some(2), dim))
                .unwrap();
            out.push(event.children.insert(99, oob_insert));
            let oob_replace = event
                .children
                .construct(spec("minecraft:slime", [0.0, 64.0, 0.0], 0.0, Some(2), dim))
                .unwrap();
            out.push(event.children.replace(99, oob_replace).map(|_| ()));
            // Unbounded list grows past the former 64-child cap (Audit [69]).
            while event.children.len() < 70 {
                let more = event
                    .children
                    .construct(spec("minecraft:slime", [0.0, 64.0, 0.0], 0.0, Some(2), dim))
                    .unwrap();
                event.children.push(more).unwrap();
            }
            out.push(
                event
                    .children
                    .get_mut(0)
                    .unwrap()
                    .set_position([f64::NAN, 0.0, 0.0]),
            );
            *record.lock().unwrap() = out;
            Flow::Continue
        },
        |game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            game.split_cube_children(&mut dying);
            assert_eq!(
                game.mobs.len(),
                70,
                "unbounded list grows past 64 and stands intact after refused operations"
            );
            assert!(game.mobs[0].position.x.is_finite());
        },
    );
    assert_eq!(
        *errors.lock().unwrap(),
        [
            Err(UnspawnedMobError::Invalid),
            Err(UnspawnedMobError::Invalid),
            Err(UnspawnedMobError::Invalid),
            Err(UnspawnedMobError::Invalid),
            Err(UnspawnedMobError::Invalid),
            Err(UnspawnedMobError::Invalid),
        ]
    );
}

#[test]
#[ignore = "daily"]
fn a_panicking_listener_spawns_nothing_and_leaves_the_arena_usable() {
    let panic_now = Arc::new(Mutex::new(true));
    let flag = panic_now.clone();
    with_split(
        move |_, event| {
            event.children.get_mut(0).unwrap().set_yaw(1.0).unwrap();
            let _detached = event
                .children
                .construct(spec(
                    "minecraft:slime",
                    [1.0, 64.0, 1.0],
                    0.0,
                    Some(2),
                    event.dimension,
                ))
                .unwrap();
            if *flag.lock().unwrap() {
                panic!("a listener fails before commit");
            }
            Flow::Continue
        },
        |game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            let next_id = game.next_entity;
            let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                game.split_cube_children(&mut dying)
            }));
            assert!(unwound.is_err());
            assert!(game.mobs.is_empty(), "no child joined");
            assert_eq!(game.next_entity, next_id, "no entity number was issued");
            // Pre-callback effects stay: the construction draws were spent.
            assert_eq!(dying.random.next_int(), NEXT_INT);
            // Subsequent splits work cleanly from a fresh stack frame.
            *panic_now.lock().unwrap() = false;
            let mut again = cube(game, "minecraft:slime", 4, SEED);
            game.split_cube_children(&mut again);
            assert_eq!(game.mobs.len(), 3);
            assert_eq!(game.mobs[0].yaw, 1.0);
        },
    );
}

// Breeding.

/// Two cows bred directly, as `breeding_announces_the_orb_before_the_lower_id_child` does: the
/// parents' ages are 6000 as the goal leaves them, the partner is in love.
pub(super) fn breed_pair(game: &mut Game<'_>, kind: &str) -> (u32, u32, Option<u32>) {
    breed_pair_credited(game, kind, false)
}

/// [`breed_pair`] with the host player credited for feeding the parents into love (what the
/// goal records in `love_cause`), when `credited`.
pub(super) fn breed_pair_credited(
    game: &mut Game<'_>,
    kind: &str,
    credited: bool,
) -> (u32, u32, Option<u32>) {
    let kind = game.res.mob_types().get(kind).unwrap().clone();
    let base = game.next_entity;
    game.next_entity += 2;
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
        game.love_cause.insert(base + 1, None);
    }
    let parent = game.mobs.iter_mut().find(|m| m.id == base + 1).unwrap();
    parent.random = LegacyRandom::new(0);
    let live_baby = !matches!(
        parent.species(),
        Species::Turtle | Species::Frog | Species::Sniffer
    );
    if mod_api::__private::listened(mod_api::HookId::BabyEntitySpawn) && live_baby {
        // Mirror the deferred initiating-parent cooldown when baby_entity_spawn is listened.
        parent.extra.age = 0;
        parent.extra.in_love = 600;
    } else {
        parent.extra.age = 6000;
    }
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

pub(super) fn find(game: &Game<'_>, id: u32) -> Mob {
    game.mobs.iter().find(|m| m.id == id).unwrap().clone()
}

#[test]
#[ignore = "daily"]
fn breeding_replaces_the_cow_baby_with_a_pig_baby() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = seen.clone();
    with_baby(
        move |_, event| {
            record.lock().unwrap().push((
                event.dimension.to_owned(),
                event.child.is_present(),
                event.child.get().unwrap().kind().unwrap().to_string(),
            ));
            let mut pig_spec = PendingMobSpec::new(
                mod_api::Identifier::parse("minecraft:pig").unwrap(),
                mod_api::Identifier::parse(event.dimension).unwrap(),
                [0.5, 64.0, 0.0],
                0.0,
                true,
            );
            pig_spec.size = None;
            let mut pig = event.child.construct(pig_spec).unwrap();
            assert_eq!(pig.set_baby(false), Err(UnspawnedMobError::Invalid));
            pig.set_yaw(15.0).unwrap();
            pig.add_tag("replaced_pig").unwrap();
            let replaced = event
                .child
                .replace(pig)
                .unwrap()
                .expect("the cow baby was in the slot");
            assert_eq!(replaced.kind().unwrap().to_string(), "minecraft:cow");
            drop(replaced);
            // A detached child dropped without being placed in the slot never joins.
            let _detached = event
                .child
                .construct(PendingMobSpec::new(
                    mod_api::Identifier::parse("minecraft:pig").unwrap(),
                    mod_api::Identifier::parse(event.dimension).unwrap(),
                    [0.5, 64.0, 0.0],
                    0.0,
                    true,
                ))
                .unwrap();
            assert_eq!(
                event.child.get_mut().unwrap().set_baby(false),
                Err(UnspawnedMobError::Invalid)
            );
            Flow::Continue
        },
        |game| {
            let (parent, partner, child) = breed_pair(game, "minecraft:cow");
            let live_parent = find(game, parent);
            let child = find(game, child.expect("a baby joined"));
            assert_eq!(child.kind.id, "minecraft:pig");
            assert!(child.extra.age < 0, "a baby has a negative age");
            // Post-event snap places the selected child at the parent's position with yaw/pitch 0.
            assert_eq!(child.position, live_parent.position);
            assert_eq!(child.yaw, 0.0);
            assert_eq!(child.head_pitch, 0.0);
            assert!(child.extra.spawn_tags.iter().any(|t| t == "replaced_pig"));
            assert_eq!(
                game.mobs
                    .iter()
                    .filter(|m| m.kind.id == "minecraft:cow")
                    .count(),
                2,
                "only the parents are cows: no cow baby"
            );
            assert_eq!(
                game.mobs
                    .iter()
                    .filter(|m| m.kind.id == "minecraft:pig")
                    .count(),
                1,
                "only the attached pig joined, not the dropped detached pig"
            );
            for id in [parent, partner] {
                let p = find(game, id);
                assert_eq!(p.extra.age, 6000);
                assert_eq!(p.extra.in_love, 0);
            }
        },
    );
    assert_eq!(
        *seen.lock().unwrap(),
        [(
            "minecraft:overworld".to_owned(),
            true,
            "minecraft:cow".to_owned()
        )]
    );
}

#[test]
#[ignore = "daily"]
fn breeding_veto_leaves_no_baby_orb_or_statistic_and_the_parents_resting() {
    with_baby(
        |_, _| Flow::Cancel,
        |game| {
            let before = game.mobs.len();
            let (parent, partner, child) = breed_pair(game, "minecraft:cow");
            assert_eq!(child, None);
            assert_eq!(game.mobs.len(), before + 2, "only the two parents exist");
            assert!(game.mobs.iter().all(|m| m.extra.age >= 0), "no baby");
            assert!(
                !game
                    .mobs
                    .iter()
                    .any(|m| m.kind.id == "minecraft:experience_orb"),
                "no experience"
            );
            for id in [parent, partner] {
                let p = find(game, id);
                assert_eq!(p.extra.age, 6000);
                assert_eq!(p.extra.in_love, 0);
            }
        },
    );
}

#[test]
#[ignore = "daily"]
fn clearing_the_baby_without_cancel_leaves_parents_and_credit_untouched() {
    with_baby(
        |_, event| {
            let taken = event.child.take();
            assert!(taken.is_some());
            assert!(event.child.is_empty());
            Flow::Continue
        },
        |game| {
            let (parent, partner, child) = breed_pair_credited(game, "minecraft:cow", true);
            assert_eq!(child, None);
            assert_eq!(animals_bred(game), 0);
            assert!(
                game.love_cause.contains_key(&parent),
                "clearing without cancel must not consume breeding credit"
            );
            assert!(game.mobs.iter().all(|m| m.extra.age >= 0));
            for id in [parent, partner] {
                let p = find(game, id);
                assert_eq!(p.extra.age, 0);
                assert_eq!(p.extra.in_love, 600);
            }
        },
    );
}

#[test]
#[ignore = "daily"]
fn breeding_dispatches_when_proposed_child_is_none_and_accepts_replacement() {
    let breed_sterile = |game: &mut Game<'_>| {
        let mut sterile = (**game.res.mob_types().get("minecraft:cow").unwrap()).clone();
        sterile.id = "test:sterile_cow".into();
        let kind = Arc::new(sterile);
        let base = game.next_entity;
        game.next_entity += 2;
        game.mobs.push(Mob::new(
            base + 1,
            kind.clone(),
            Vec3::new(3.0, 64.0, 4.0),
            0.0,
        ));
        game.mobs
            .push(Mob::new(base + 2, kind, Vec3::new(4.0, 64.0, 4.0), 0.0));
        game.native_joins_skip();
        for id in [base + 1, base + 2] {
            let m = game.mobs.iter_mut().find(|m| m.id == id).unwrap();
            m.random = LegacyRandom::new(0);
            m.extra.age = 0;
            m.extra.in_love = 600;
        }
        let child = game.breed(
            base + 1,
            base + 2,
            None,
            None,
            None,
            sunrise_client::mobs::BreedExperience::Deferred,
        );
        (base + 1, base + 2, child)
    };
    // Populating an initially empty slot spawns the replacement and resets both parents.
    with_baby(
        |_, event| {
            assert!(event.child.is_empty());
            let mut pig = event
                .child
                .construct(PendingMobSpec::new(
                    mod_api::Identifier::parse("minecraft:pig").unwrap(),
                    mod_api::Identifier::parse(event.dimension).unwrap(),
                    [9.0, 70.0, 9.0],
                    45.0,
                    true,
                ))
                .unwrap();
            pig.add_tag("from_empty_slot").unwrap();
            assert!(event.child.replace(pig).unwrap().is_none());
            Flow::Continue
        },
        |game| {
            let (parent, partner, child) = breed_sterile(game);
            let baby = find(game, child.expect("populated empty slot should spawn"));
            assert_eq!(baby.kind.id, "minecraft:pig");
            assert_eq!(baby.position, Vec3::new(3.0, 64.0, 4.0));
            assert_eq!(baby.yaw, 0.0);
            assert!(baby.extra.spawn_tags.iter().any(|t| t == "from_empty_slot"));
            for id in [parent, partner] {
                let p = find(game, id);
                assert_eq!(p.extra.age, 6000);
                assert_eq!(p.extra.in_love, 0);
            }
        },
    );
    // Leaving an initially empty slot empty without cancelling skips finalization and cooldown.
    with_baby(
        |_, event| {
            assert!(event.child.is_empty());
            Flow::Continue
        },
        |game| {
            let (parent, partner, child) = breed_sterile(game);
            assert_eq!(child, None);
            for id in [parent, partner] {
                let p = find(game, id);
                assert_eq!(p.extra.age, 0);
                assert_eq!(p.extra.in_love, 600);
            }
        },
    );
    // Cancelling an initially empty slot resets both parents.
    with_baby(
        |_, event| {
            assert!(event.child.is_empty());
            Flow::Cancel
        },
        |game| {
            let (parent, partner, child) = breed_sterile(game);
            assert_eq!(child, None);
            for id in [parent, partner] {
                let p = find(game, id);
                assert_eq!(p.extra.age, 6000);
                assert_eq!(p.extra.in_love, 0);
            }
        },
    );
}

#[test]
#[ignore = "daily"]
fn fox_breeding_rejects_non_fox_and_trusts_breeders_on_replaced_fox() {
    with_baby(
        |_, event| {
            assert_eq!(
                event
                    .child
                    .construct(PendingMobSpec::new(
                        mod_api::Identifier::parse("minecraft:cow").unwrap(),
                        mod_api::Identifier::parse(event.dimension).unwrap(),
                        [0.0, 64.0, 0.0],
                        0.0,
                        true,
                    ))
                    .err(),
                Some(UnspawnedMobError::Invalid)
            );
            let mut kit = event
                .child
                .construct(PendingMobSpec::new(
                    mod_api::Identifier::parse("minecraft:fox").unwrap(),
                    mod_api::Identifier::parse(event.dimension).unwrap(),
                    [5.0, 70.0, 5.0],
                    42.0,
                    true,
                ))
                .unwrap();
            kit.add_tag("custom_kit").unwrap();
            let prev = event.child.replace(kit).unwrap();
            assert_eq!(prev.unwrap().kind().unwrap().to_string(), "minecraft:fox");
            Flow::Continue
        },
        |game| {
            game.player =
                Some(sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
            let feeder = sunrise_client::lan::playerdata::uuid_ints(game.native_acting_id());
            let (parent, partner, child) = breed_pair_credited(game, "minecraft:fox", true);
            let live_parent = find(game, parent);
            let kit = find(game, child.expect("replaced fox kit should join"));
            assert_eq!(kit.kind.id, "minecraft:fox");
            assert!(kit.extra.age < 0);
            assert!(kit.extra.spawn_tags.iter().any(|t| t == "custom_kit"));
            assert!(sunrise_client::mobs::fox::trusts(&kit, feeder));
            assert_eq!(kit.position, live_parent.position);
            assert_eq!(kit.yaw, 0.0);
            assert_eq!(kit.head_pitch, 0.0);
            for id in [parent, partner] {
                let p = find(game, id);
                assert_eq!(p.extra.age, 6000);
                assert_eq!(p.extra.in_love, 0);
            }
        },
    );
}

#[test]
#[ignore = "daily"]
fn egg_layers_never_ask_baby_entity_spawn() {
    let calls = Arc::new(Mutex::new(0u32));
    let count = calls.clone();
    with_baby(
        move |_, _| {
            *count.lock().unwrap() += 1;
            Flow::Continue
        },
        |game| {
            for kind in ["minecraft:turtle", "minecraft:frog", "minecraft:sniffer"] {
                game.mobs.clear();
                let _ = breed_pair(game, kind);
            }
            // A live baby, by contrast, is asked once.
            game.mobs.clear();
            let _ = breed_pair(game, "minecraft:cow");
        },
    );
    assert_eq!(*calls.lock().unwrap(), 1, "only the cow's baby was asked");
}

#[test]
#[ignore = "daily"]
fn breeding_without_a_listener_is_vanilla() {
    let mods: mod_api::Mods = Vec::new().into();
    mod_platform::testing::with_mods(mods, || {
        with_game(|game| {
            let (parent, partner, child) = breed_pair(game, "minecraft:cow");
            let baby = find(game, child.expect("a baby joined"));
            assert_eq!(baby.kind.id, "minecraft:cow");
            assert_eq!(baby.extra.age, -24000);
            for id in [parent, partner] {
                assert_eq!(find(game, id).extra.age, 6000);
            }
        });
    });
}

#[test]
#[ignore = "daily"]
fn turtle_hatching_spawns_its_turtle_without_asking_baby_entity_spawn() {
    let calls = Arc::new(Mutex::new(0u32));
    let count = calls.clone();
    with_baby(
        move |_, _| {
            *count.lock().unwrap() += 1;
            Flow::Continue
        },
        |game| {
            let egg = game
                .pack
                .assets
                .registry
                .default_state("minecraft:turtle_egg")
                .unwrap();
            game.mobs.clear();
            game.hatch_turtles([5, 64, 5], egg);
            // One egg: one baby turtle, its home the egg, and no birth hook.
            assert_eq!(game.mobs.len(), 1);
            assert_eq!(game.mobs[0].kind.id, "minecraft:turtle");
            assert_eq!(game.mobs[0].extra.age, -24000);
            assert_eq!(game.mobs[0].extra.home, Some([5, 64, 5]));
        },
    );
    assert_eq!(*calls.lock().unwrap(), 0);
}

fn with_splits(listeners: Vec<SplitFn>, run: impl FnOnce(&mut Game<'_>)) {
    let entries: Vec<_> = listeners
        .into_iter()
        .enumerate()
        .map(|(i, listener)| {
            mod_api::ModEntry::new(
                Box::leak(format!("split{i}").into_boxed_str()),
                Box::new(Split(listener)),
            )
        })
        .collect();
    mod_platform::testing::with_mods(entries.into(), || with_game(run));
}

fn animals_bred(game: &Game<'_>) -> i32 {
    game.stats.get("custom", "minecraft:animals_bred")
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
            assert!(!game.love_cause.contains_key(&parent));
            assert!(!game.love_cause.contains_key(&partner));
        },
    );
    with_baby(
        |_, _| Flow::Cancel,
        |game| {
            let (parent, partner, child) = breed_pair_credited(game, "minecraft:cow", true);
            assert_eq!(child, None);
            assert_eq!(animals_bred(game), 0, "a vetoed baby earns no statistic");
            assert!(
                !game.love_cause.contains_key(&parent),
                "the credit is spent"
            );
            assert!(!game.love_cause.contains_key(&partner));
        },
    );
}

#[test]
#[ignore = "daily"]
fn a_waiting_baby_publishes_its_uuid_only_when_it_joins() {
    with_baby(
        |_, _| Flow::Continue,
        |game| {
            let (parent, _, child) = breed_pair(game, "minecraft:cow");
            let baby = child.unwrap();
            assert_eq!(baby, parent + 2);
            assert!(game.persist.uuids.contains_key(&baby), "published at join");
        },
    );
    with_baby(
        |_, event| {
            event.child.clear();
            assert!(event.child.is_empty());
            Flow::Continue
        },
        |game| {
            let (parent, _, child) = breed_pair(game, "minecraft:cow");
            assert_eq!(child, None);
            assert!(
                !game.persist.uuids.contains_key(&(parent + 2)),
                "a removed baby never published one"
            );
        },
    );
}

#[test]
#[ignore = "daily"]
fn a_panicking_baby_listener_leaves_no_reserved_state_behind() {
    with_baby(
        |_, event| {
            let _detached = event
                .child
                .construct(PendingMobSpec::new(
                    mod_api::Identifier::parse("minecraft:pig").unwrap(),
                    mod_api::Identifier::parse(event.dimension).unwrap(),
                    [0.5, 64.0, 0.0],
                    0.0,
                    true,
                ))
                .unwrap();
            panic!("a listener fails")
        },
        |game| {
            let (uuids, attachments) = (game.persist.uuids.len(), game.entity_attachments.len());
            let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                breed_pair(game, "minecraft:cow")
            }));
            assert!(unwound.is_err());
            assert_eq!(game.persist.uuids.len(), uuids, "the baby's UUID is gone");
            assert_eq!(game.entity_attachments.len(), attachments);
            assert!(game.mobs.iter().all(|m| m.extra.age >= 0), "no baby joined");
        },
    );
}

/// Touching a parent via `host` during dispatch no longer aborts the birth, and cancelling
/// resets both `parent` and `partner` to `age = 6000` and `in_love = 0` (matching NeoForge's
/// `Animal.spawnChildFromBreeding`) even if a listener edited parent love/age.
#[test]
#[ignore = "daily"]
fn breeding_spawns_after_parent_host_edit_and_cancel_resets_both_parents() {
    with_baby(
        |host, event| {
            let parent = host.entity(event.parent).expect("parent handle");
            assert!(parent.teleport_to(host, [2.0, 64.0, 0.0], None));
            let mut nbt = mod_api::Compound::new();
            nbt.insert("InLove".into(), mod_api::Tag::Int(600));
            assert!(parent.merge_nbt(host, nbt));
            Flow::Continue
        },
        |game| {
            let (parent, partner, child) = breed_pair(game, "minecraft:cow");
            let baby = find(game, child.expect("parent host edit must not abort birth"));
            assert_eq!(baby.kind.id, "minecraft:cow");
            assert_eq!(baby.extra.age, -24000);
            assert_eq!(find(game, parent).position.x, 2.0);
            // Post-event snap tracks the live parent after listener teleport and zeroes yaw/pitch.
            assert_eq!(baby.position, Vec3::new(2.0, 64.0, 0.0));
            assert_eq!(baby.yaw, 0.0);
            assert_eq!(baby.head_pitch, 0.0);
            for id in [parent, partner] {
                let mob = find(game, id);
                assert_eq!(mob.extra.age, 6000);
                assert_eq!(mob.extra.in_love, 0);
            }
        },
    );
    with_baby(
        |host, event| {
            for id in [event.parent, event.partner] {
                let entity = host.entity(id).expect("parent handle");
                let mut nbt = mod_api::Compound::new();
                nbt.insert("Age".into(), mod_api::Tag::Int(0));
                nbt.insert("InLove".into(), mod_api::Tag::Int(300));
                assert!(entity.merge_nbt(host, nbt));
            }
            Flow::Cancel
        },
        |game| {
            let (parent, partner, child) = breed_pair(game, "minecraft:cow");
            assert_eq!(child, None);
            assert!(game.mobs.iter().all(|m| m.extra.age >= 0), "no baby joined");
            for id in [parent, partner] {
                let mob = find(game, id);
                assert_eq!(mob.extra.age, 6000);
                assert_eq!(mob.extra.in_love, 0);
            }
        },
    );
}

#[test]
#[ignore = "daily"]
fn detached_constructed_baby_preserves_attachments_and_reattaches_across_worlds_without_collision()
{
    const PENDING_MARKER: AttachmentType<i32> = AttachmentType {
        namespace: "pending_birth",
        name: "marker",
        default: || 0,
        codec: None,
        copy_on_death: false,
        sync: false,
    };
    thread_local! {
        static STASH: std::cell::RefCell<Option<UnspawnedMob>> =
            const { std::cell::RefCell::new(None) };
    }
    struct CrossWorldMod {
        construct_calls: Arc<Mutex<Vec<(Holder, String)>>>,
        attach_on_split: Arc<Mutex<bool>>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for CrossWorldMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            r.register_attachment_type("pending_birth:marker", false)
                .unwrap();
        }
        fn entity_constructing(&self, host: HostSide<'_>, event: &mut EntityConstructingEvent) {
            self.construct_calls
                .lock()
                .unwrap()
                .push((event.holder, event.kind.to_owned()));
            match host {
                HostSide::Server(host) => {
                    PENDING_MARKER.set(host, event.holder, 41).unwrap();
                }
                HostSide::Client(host) => {
                    PENDING_MARKER.set(host, event.holder, 41).unwrap();
                }
            }
        }
        fn baby_entity_spawn(
            &self,
            _: &mut dyn ServerHost,
            event: &mut BabyEntitySpawnEvent<'_>,
        ) -> Flow {
            let taken = event.child.take();
            STASH.with(|slot| *slot.borrow_mut() = taken);
            Flow::Continue
        }
        fn mob_split(&self, _: &mut dyn ServerHost, event: &mut MobSplitEvent<'_>) -> Flow {
            if *self.attach_on_split.lock().unwrap() {
                event.children.clear();
                let stashed = STASH.with(|slot| slot.borrow_mut().take()).unwrap();
                event.children.push(stashed).unwrap();
            }
            Flow::Continue
        }
    }

    STASH.with(|slot| *slot.borrow_mut() = None);
    let construct_calls = Arc::new(Mutex::new(Vec::new()));
    let attach_on_split = Arc::new(Mutex::new(false));
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "pending_birth_cross_world",
        Box::new(CrossWorldMod {
            construct_calls: construct_calls.clone(),
            attach_on_split: attach_on_split.clone(),
        }),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        // World 1: construct a breeding cow baby (running entity_constructing -> marker=41),
        // then take it out of the slot without spawning it.
        let old_reserved_id = {
            let mut reserved = 0;
            with_game(|game| {
                let (parent, _, child) = breed_pair(game, "minecraft:cow");
                assert_eq!(child, None);
                reserved = parent + 2;
                assert!(
                    !game.persist.uuids.contains_key(&reserved),
                    "detached baby must not leak UUID in World 1"
                );
                assert!(
                    !game.entity_attachments.contains_key(&reserved),
                    "detached baby must not leak attachments in World 1"
                );
                assert!(
                    !game
                        .entity_initialization
                        .initialized
                        .contains(&Holder::Entity(reserved)),
                    "detached baby must not leak initialization marker in World 1"
                );
                assert!(game.entity_initialization.active.is_none());
            });
            reserved
        };
        assert_eq!(
            construct_calls
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, k)| k == "minecraft:cow")
                .count(),
            1,
            "cow baby constructed once in World 1"
        );

        // World 2: place a resident sheep at the exact same entity ID as World 1's old
        // reserved ID, then reattach the stashed cow baby via mob_split.
        *attach_on_split.lock().unwrap() = true;
        with_game(|game| {
            let sheep_kind = game.res.mob_types().get("minecraft:sheep").unwrap().clone();
            game.next_entity = old_reserved_id;
            game.mobs.push(Mob::new(
                old_reserved_id,
                sheep_kind,
                Vec3::new(1.0, 64.0, 1.0),
                0.0,
            ));
            game.persist.uuids.insert(old_reserved_id, [9, 9, 9, 9]);
            game.native_joins_skip();

            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            game.split_cube_children(&mut dying);

            // Resident sheep at old_reserved_id is completely untouched.
            let sheep = find(game, old_reserved_id);
            assert_eq!(sheep.kind.id, "minecraft:sheep");
            assert_eq!(
                game.persist.uuids.get(&old_reserved_id),
                Some(&[9, 9, 9, 9])
            );

            // Reattached cow received a fresh ID, kept its constructor attachment (41),
            // and was NOT double-constructed.
            let cow = game
                .mobs
                .iter()
                .find(|m| m.kind.id == "minecraft:cow")
                .expect("reattached cow joined World 2")
                .clone();
            assert_ne!(cow.id, old_reserved_id);
            assert!(game.persist.uuids.contains_key(&cow.id));
            assert_eq!(PENDING_MARKER.get(game, Holder::Entity(cow.id)), Ok(41));
        });
        assert_eq!(
            construct_calls
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, k)| k == "minecraft:cow")
                .count(),
            1,
            "reattached cow must not run entity_constructing a second time"
        );
    });
    STASH.with(|slot| *slot.borrow_mut() = None);
}

#[test]
#[ignore = "daily"]
fn mob_split_constructs_all_children_before_any_join_and_cleans_up_on_unwind() {
    const SPLIT_MARKER: AttachmentType<i32> = AttachmentType {
        namespace: "pending_split",
        name: "marker",
        default: || 0,
        codec: None,
        copy_on_death: false,
        sync: false,
    };
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum PanicAt {
        None,
        SecondConstruct,
        SecondJoin,
    }
    struct OrderMod {
        trace: Arc<Mutex<Vec<(&'static str, u32)>>>,
        panic_at: Arc<Mutex<PanicAt>>,
        constructs: Arc<Mutex<usize>>,
        joins: Arc<Mutex<usize>>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for OrderMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            r.register_attachment_type("pending_split:marker", false)
                .unwrap();
        }
        fn mob_split(&self, _: &mut dyn ServerHost, _: &mut MobSplitEvent<'_>) -> Flow {
            Flow::Continue
        }
        fn entity_constructing(&self, host: HostSide<'_>, event: &mut EntityConstructingEvent) {
            if let Holder::Entity(id) = event.holder {
                self.trace.lock().unwrap().push(("construct", id));
                if let HostSide::Server(host) = host {
                    SPLIT_MARKER.set(host, event.holder, 7).unwrap();
                }
                let mut count = self.constructs.lock().unwrap();
                *count += 1;
                if *self.panic_at.lock().unwrap() == PanicAt::SecondConstruct && *count == 2 {
                    panic!("construct unwind on second split child");
                }
            }
        }
        fn entity_join_level(
            &self,
            _: &mut dyn ServerHost,
            event: &mut EntityJoinLevelEvent<'_>,
        ) -> Flow {
            self.trace.lock().unwrap().push(("join", event.entity));
            let mut count = self.joins.lock().unwrap();
            *count += 1;
            if *self.panic_at.lock().unwrap() == PanicAt::SecondJoin && *count == 2 {
                panic!("join unwind on second split child");
            }
            Flow::Continue
        }
    }

    let trace = Arc::new(Mutex::new(Vec::new()));
    let panic_at = Arc::new(Mutex::new(PanicAt::None));
    let constructs = Arc::new(Mutex::new(0));
    let joins = Arc::new(Mutex::new(0));
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "pending_split_order",
        Box::new(OrderMod {
            trace: trace.clone(),
            panic_at: panic_at.clone(),
            constructs: constructs.clone(),
            joins: joins.clone(),
        }),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_game(|game| {
            // 1. Normal split: all 3 children construct in order before the first joins.
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            let first = game.next_entity + 1;
            game.split_cube_children(&mut dying);
            assert_eq!(game.mobs.len(), 3);
            assert_eq!(
                *trace.lock().unwrap(),
                vec![
                    ("construct", first),
                    ("construct", first + 1),
                    ("construct", first + 2),
                    ("join", first),
                    ("join", first + 1),
                    ("join", first + 2),
                ]
            );

            // 2. Panic during 2nd child's entity_constructing: zero children join,
            // active construction holder is restored, and unjoined children's state is cleaned up.
            game.mobs.clear();
            trace.lock().unwrap().clear();
            *constructs.lock().unwrap() = 0;
            *joins.lock().unwrap() = 0;
            *panic_at.lock().unwrap() = PanicAt::SecondConstruct;
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            let before = game.next_entity;
            let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                game.split_cube_children(&mut dying)
            }));
            assert!(unwound.is_err());
            assert!(game.mobs.is_empty());
            assert!(game.entity_initialization.active.is_none());
            for id in (before + 1)..=game.next_entity {
                assert!(!game.persist.uuids.contains_key(&id));
                assert!(!game.entity_attachments.contains_key(&id));
                assert!(
                    !game
                        .entity_initialization
                        .initialized
                        .contains(&Holder::Entity(id))
                );
            }

            // 3. Panic during 2nd child's entity_join_level: 1st child remains joined,
            // and the 3rd (unjoined) child's issued state is cleaned up.
            game.mobs.clear();
            trace.lock().unwrap().clear();
            *constructs.lock().unwrap() = 0;
            *joins.lock().unwrap() = 0;
            *panic_at.lock().unwrap() = PanicAt::SecondJoin;
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            let first = game.next_entity + 1;
            let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                game.split_cube_children(&mut dying)
            }));
            assert!(unwound.is_err());
            assert!(game.entity_initialization.active.is_none());
            assert!(game.mobs.iter().any(|m| m.id == first));
            let third = first + 2;
            assert!(!game.mobs.iter().any(|m| m.id == third));
            assert!(!game.entity_attachments.contains_key(&third));
            assert!(
                !game
                    .entity_initialization
                    .initialized
                    .contains(&Holder::Entity(third))
            );
        });
    });
}
