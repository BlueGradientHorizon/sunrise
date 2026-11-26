//! Real-route fixtures for the pending-children hooks: `mob_split` through the slime/magma/sulfur
//! split and `baby_entity_spawn` through breeding. The expected values are
//! written out: the seeds' draws come from Java's `java.util.Random` (`new Random(2)` gives
//! `nextInt(3) = 1`, so three children; then three `nextFloat() * 360.0F` yaws;
//! see `YAWS`), and the offsets are vanilla's `setUpSplitCube` arithmetic spelled out in the test.
//!
//! They need prepared resources (`SUNRISE_TEST_RESOURCES`) and run with a native test mod
//! installed for the test only.

use {crate::game::creatures::*, sunrise_gameplay_common::game::creatures::{Dim, Mob, breeding, seen}};
use sunrise_gameplay_common::world_biomes;
use {sunrise_gameplay::game::native_entity_refs::tests::run_fresh_cases, sunrise_gameplay::game::native_entity_refs::tests::with_game};
use mod_api::{
    __private::{HookPanicReport, take_hook_panics},
    BabyEntitySpawnEvent, Flow, Hooks, MobSplitEvent, PendingMobSpec, ServerHost,
    UnspawnedMobError,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, AtomicUsize, Ordering},
};
use sunrise_level::chunk::Chunk;

pub(crate) const IGNORE: &str = "needs prepared resources in SUNRISE_TEST_RESOURCES";

/// `new Random(2)`: `2 + nextInt(3)` is 3, then `nextFloat() * 360.0F` three times, then the
/// next `nextInt()`. The same three draws with no listener, a cancel or a listener.
pub(crate) use sunrise_gameplay::game::server_side::creatures_server::pending_birth_matrix_tests::SEED;
pub(crate) const YAWS: [f32; 3] = [105.61554, 324.52115, 1.4961791];
pub(crate) const NEXT_INT: i32 = 2_133_836_778;

pub(crate) type SplitFn =
    Box<dyn Fn(&mut dyn ServerHost, &mut MobSplitEvent<'_>) -> Flow + Send + Sync>;
pub(crate) struct Split(pub(crate) SplitFn);
#[mod_api::__subscriptions]
impl Hooks for Split {
    fn mob_split(&self, host: &mut dyn ServerHost, event: &mut MobSplitEvent<'_>) -> Flow {
        (self.0)(host, event)
    }
}

pub(crate) type BabyFn =
    Box<dyn Fn(&mut dyn ServerHost, &mut BabyEntitySpawnEvent<'_>) -> Flow + Send + Sync>;
pub(crate) struct Baby(pub(crate) BabyFn);
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

pub(crate) fn with_split(
    listener: impl Fn(&mut dyn ServerHost, &mut MobSplitEvent<'_>) -> Flow + Send + Sync + 'static,
    run: impl FnOnce(&mut sunrise_gameplay::game::ServerGame<'_>),
) {
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "split",
        Box::new(Split(Box::new(listener))),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || with_game(run));
}

pub(crate) fn with_baby(
    listener: impl Fn(&mut dyn ServerHost, &mut BabyEntitySpawnEvent<'_>) -> Flow
    + Send
    + Sync
    + 'static,
    run: impl FnOnce(&mut sunrise_gameplay::game::ServerGame<'_>),
) {
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "baby",
        Box::new(Baby(Box::new(listener))),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || with_game(run));
}

pub(crate) use sunrise_gameplay::game::server_side::creatures_server::pending_birth_matrix_tests::AT;

#[allow(unused_imports)]
pub(crate) use crate::game::server_side::creatures_server::pending_birth_tests::cube;

pub(crate) fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-4
}

/// The mobs the split added, in order (the fixture world starts with none).
pub(crate) fn spawned(game: &sunrise_gameplay::game::ServerGame<'_>) -> Vec<Mob> {
    game.mobs.to_vec()
}

pub(crate) fn assert_vanilla_children(children: &[Mob], kind: &str, first_id: u32) {
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
fn sulfur_cube_splits_two_babies_unless_primed_and_size_one_asks_nobody() {
    let calls = Arc::new(Mutex::new(0u32));
    let count = calls.clone();
    with_split(
        move |_, _| {
            *count.lock().unwrap() += 1;
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
            assert_eq!(*calls.lock().unwrap(), 1);
            // Primed: no children, nobody asked.
            game.mobs.clear();
            let mut dying = cube(game, "minecraft:sulfur_cube", 4, 0);
            dying.extra.sulfur_fuse = sunrise_sim::mobs::sulfur_cube::Fuse::set(40);
            assert!(sunrise_sim::mobs::sulfur_cube::primed(&dying));
            game.split_dead_cube(dying);
            assert!(game.mobs.is_empty());
            assert_eq!(*calls.lock().unwrap(), 1);
            // A size-1 parent never splits.
            let dying = cube(game, "minecraft:slime", 1, 0);
            game.split_dead_cube(dying);
            assert!(game.mobs.is_empty());
            assert_eq!(*calls.lock().unwrap(), 1);
        },
    );
}

pub(crate) fn spec(
    kind: &str,
    at: [f64; 3],
    yaw: f32,
    size: Option<u32>,
    dimension: &str,
) -> PendingMobSpec {
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
fn replace_c_with_e_commits_a_b_e_and_a_second_append_of_e_is_an_ownership_error() {
    let outcome = Arc::new(Mutex::new(None));
    let record = outcome.clone();
    with_split(
        move |host, event| {
            let (edits, children) = (event.edits, event.edits.children().unwrap());
            let c = edits.child(children, 2).unwrap();
            let e = edits
                .construct(
                    children,
                    spec(
                        "minecraft:magma_cube",
                        [8.0, 65.0, 9.0],
                        33.0,
                        Some(2),
                        event.dimension,
                    ),
                )
                .unwrap();
            edits.replace(children, 2, e).unwrap();
            let again = edits.append(children, e);
            *record.lock().unwrap() = Some((c.kind(&*host), again));
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
    let (replaced, again) = outcome.lock().unwrap().clone().unwrap();
    assert_eq!(replaced, Err(UnspawnedMobError::Expired), "C is invalid");
    assert_eq!(
        again,
        Err(UnspawnedMobError::Ownership),
        "E is consumed once"
    );
}

#[test]
#[ignore = "daily"]
fn invalid_input_fails_before_mutation_and_the_valid_list_stands() {
    let errors = Arc::new(Mutex::new(Vec::new()));
    let record = errors.clone();
    with_split(
        move |_, event| {
            let (edits, children) = (event.edits, event.edits.children().unwrap());
            let dim = event.dimension;
            let mut out = Vec::new();
            out.push(
                edits
                    .construct(
                        children,
                        spec("minecraft:slime", [f64::NAN, 64.0, 0.0], 0.0, Some(2), dim),
                    )
                    .map(|_| ()),
            );
            out.push(
                edits
                    .construct(
                        children,
                        spec("minecraft:not_a_mob", [0.0, 64.0, 0.0], 0.0, None, dim),
                    )
                    .map(|_| ()),
            );
            out.push(
                edits
                    .construct(
                        children,
                        spec(
                            "minecraft:slime",
                            [0.0, 64.0, 0.0],
                            0.0,
                            Some(2),
                            "minecraft:the_end",
                        ),
                    )
                    .map(|_| ()),
            );
            // A duplicate token: the second append is refused.
            let d = edits
                .construct(
                    children,
                    spec("minecraft:slime", [0.0, 64.0, 0.0], 0.0, Some(2), dim),
                )
                .unwrap();
            edits.append(children, d).unwrap();
            out.push(edits.append(children, d));
            // Up to 64, then the 65th child is a capacity error.
            while edits.len(children).unwrap() < 64 {
                let more = edits
                    .construct(
                        children,
                        spec("minecraft:slime", [0.0, 64.0, 0.0], 0.0, Some(2), dim),
                    )
                    .unwrap();
                edits.append(children, more).unwrap();
            }
            let extra = edits
                .construct(
                    children,
                    spec("minecraft:slime", [0.0, 64.0, 0.0], 0.0, Some(2), dim),
                )
                .unwrap();
            out.push(edits.append(children, extra));
            out.push(edits.set_position(edits.child(children, 0).unwrap(), [f64::NAN, 0.0, 0.0]));
            *record.lock().unwrap() = out;
            Flow::Continue
        },
        |game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            game.split_cube_children(&mut dying);
            assert_eq!(
                game.mobs.len(),
                64,
                "the valid list stands, nothing truncated"
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
            Err(UnspawnedMobError::Ownership),
            Err(UnspawnedMobError::Capacity),
            Err(UnspawnedMobError::Invalid),
        ]
    );
}

#[test]
#[ignore = "daily"]
fn retained_native_tokens_fail_after_the_callback_returns() {
    let kept = Arc::new(Mutex::new(None));
    let record = kept.clone();
    with_split(
        move |_, event| {
            let child = event
                .edits
                .child(event.edits.children().unwrap(), 0)
                .unwrap();
            *record.lock().unwrap() = Some((event.edits.children().unwrap(), child));
            Flow::Continue
        },
        |game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            game.split_cube_children(&mut dying);
            let (_, child) = kept.lock().unwrap().unwrap();
            // Reads and writes through the retained borrow change nothing and are refused.
            assert_eq!(
                game.unspawned_mob_kind(child),
                Err(UnspawnedMobError::Expired)
            );
            assert_eq!(
                game.unspawned_mob_position(child),
                Err(UnspawnedMobError::Expired)
            );
            // The children that joined were not retargeted by the stale handle.
            assert_eq!(game.mobs.len(), 3);
            assert_eq!(game.mobs[0].yaw, YAWS[0]);
        },
    );
}

pub(crate) struct NetherBiomeOnJoin {
    pub(crate) biome: String,
    pub(crate) writes: Arc<AtomicUsize>,
}

#[mod_api::__subscriptions]
impl Hooks for NetherBiomeOnJoin {
    fn baby_entity_spawn(
        &self,
        host: &mut dyn ServerHost,
        _: &mut BabyEntitySpawnEvent<'_>,
    ) -> Flow {
        assert!(
            host.commands()
                .run_command("execute in minecraft:the_nether run summon minecraft:pig 4 64 4")
        );
        assert_eq!(self.writes.load(Ordering::SeqCst), 1);
        panic!("abort the Overworld birth");
    }

    fn entity_join_level(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::EntityJoinLevelEvent<'_>,
    ) -> Flow {
        if event.kind == "minecraft:pig" {
            assert_eq!(event.dimension, Dim::Nether.id());
            let level = host.level(event.dimension).unwrap();
            assert!(level.set_biome(host, [4, 64, 4], &self.biome));
            self.writes.fetch_add(1, Ordering::SeqCst);
        }
        Flow::Continue
    }
}

#[test]
#[ignore = "daily"]
fn a_birth_panic_does_not_undo_or_misroute_a_biome_write_in_another_level() {
    use sunrise_protocol::{buf::Reader, packets::play::Clientbound};
    use sunrise_sim::lan::{Out, PlayerSender};

    if run_fresh_cases(
        "game::creatures::pending_birth_tests::a_birth_panic_does_not_undo_or_misroute_a_biome_write_in_another_level",
        "SUNRISE_BIRTH_BIOME_CHILD",
        &["biome"],
    ) {
        return;
    }
    sunrise_gameplay::game::native_guest_turn_tests::with_guests(|game| {
        game.build_registries();
        let key = (0, 0);
        let (overworld, nether, changed) = (0_u16, 1_u16, 2_u16);
        for (dim, biome) in [(Dim::Nether, nether), (Dim::Overworld, overworld)] {
            game.enter_level(dim).unwrap();
            let mut chunk = Chunk::with_layout(0, 0, game.world.layout());
            chunk.set_biome_cell(1, (64 - game.world.bottom_y()) >> 2, 1, biome);
            game.world.chunks.insert(key, Arc::new(chunk));
            game.world.modified.clear();
            game.world.dirty_sections.clear();
        }
        let (tx, rx) = std::sync::mpsc::channel();
        game.server
            .lan
            .as_mut()
            .unwrap()
            .hub
            .update_guest(12, |guest| {
                guest.dimension = Dim::Nether.id().into();
                guest.sender = PlayerSender::new(tx, Arc::default());
                guest.sent_chunks.insert(key);
            })
            .unwrap();
        let biome = world_biomes!(game).list[usize::from(changed)].name.clone();
        let wire_biome = game
            .server
            .lan
            .as_ref()
            .unwrap()
            .hub
            .registry_id("minecraft:worldgen/biome", &biome)
            .unwrap();
        let writes = Arc::new(AtomicUsize::new(0));
        let mods = vec![mod_api::ModEntry::new(
            "nether-biome",
            Box::new(NetherBiomeOnJoin {
                biome,
                writes: writes.clone(),
            }),
        )];
        mod_platform::testing::with_mods(mods.into(), || {
            assert_eq!(breed_pair(game, "minecraft:cow").2, None);
            assert_eq!(writes.load(Ordering::SeqCst), 1);
            assert_eq!(
                take_hook_panics(),
                [HookPanicReport {
                    mod_id: "nether-biome",
                    hook: "baby_entity_spawn",
                }]
            );
        });
        assert_eq!(game.dim, Dim::Overworld);
        assert_eq!(game.world.chunks[&key].biome(4, 64, 4), overworld);
        assert!(!game.world.modified.contains(&key));
        let resends = rx
            .try_iter()
            .filter_map(|out| {
                let Out::Packet(body) = out else { return None };
                let mut reader = Reader::new(&body);
                let id = reader.varint().unwrap();
                match Clientbound::decode(id, reader.rest()).unwrap() {
                    Clientbound::ChunksBiomes(packet) => Some(packet),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        let [resend] = resends.as_slice() else {
            panic!("the Nether watcher must receive exactly one biome update");
        };
        let [column] = resend.chunks.as_slice() else {
            panic!("one biome column changed");
        };
        assert_eq!(column.pos, [0, 0]);

        game.enter_level(Dim::Nether).unwrap();
        let sections = column
            .biomes((game.world.shape.height >> 4) as usize, 8)
            .unwrap();
        let section = ((64 - game.world.shape.min_y) >> 4) as usize;
        // Local quart (1, 0, 1) in that section.
        assert_eq!(sections[section].get(5), wire_biome);
        assert_eq!(game.world.chunks[&key].biome(4, 64, 4), changed);
        assert!(
            game.world.modified.contains(&key),
            "the biome is marked for save"
        );
        assert!(
            game.world.dirty_sections.contains(&(0, 0, 0)),
            "the column was dirtied"
        );
    });
}

// Breeding.

/// Two cows bred directly, as `breeding_announces_the_orb_before_the_lower_id_child` does: the
/// parents' ages are 6000 as the goal leaves them, the partner is in love.
pub(crate) use sunrise_gameplay::game::server_side::creatures_server::pending_birth_matrix_tests::breed_pair;

#[allow(unused_imports)]
pub(crate) use crate::game::server_side::creatures_server::pending_birth_tests::breed_pair_credited;

pub(crate) fn find(game: &sunrise_gameplay::game::ServerGame<'_>, id: u32) -> Mob {
    game.mobs.iter().find(|m| m.id == id).unwrap().clone()
}

#[test]
#[ignore = "daily"]
fn breeding_replaces_the_cow_baby_with_a_pig_baby() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = seen.clone();
    with_baby(
        move |host, event| {
            let (edits, children) = (event.edits, event.edits.children().unwrap());
            record.lock().unwrap().push((
                event.dimension.to_owned(),
                edits.len(children).unwrap(),
                edits
                    .child(children, 0)
                    .unwrap()
                    .kind(&*host)
                    .unwrap()
                    .to_string(),
            ));
            let mut pig = PendingMobSpec::new(
                mod_api::Identifier::parse("minecraft:pig").unwrap(),
                mod_api::Identifier::parse(event.dimension).unwrap(),
                [0.5, 64.0, 0.0],
                0.0,
                true,
            );
            pig.size = None;
            let pig = edits.construct(children, pig).unwrap();
            edits.replace(children, 0, pig).unwrap();
            // The cow's replacement may not be joined by a second baby.
            let second = edits
                .construct(
                    children,
                    PendingMobSpec::new(
                        mod_api::Identifier::parse("minecraft:pig").unwrap(),
                        mod_api::Identifier::parse(event.dimension).unwrap(),
                        [0.5, 64.0, 0.0],
                        0.0,
                        true,
                    ),
                )
                .unwrap();
            assert_eq!(
                edits.append(children, second),
                Err(UnspawnedMobError::Capacity)
            );
            Flow::Continue
        },
        |game| {
            let (parent, partner, child) = breed_pair(game, "minecraft:cow");
            let child = find(game, child.expect("a baby joined"));
            assert_eq!(child.kind.id, "minecraft:pig");
            assert!(child.extra.age < 0, "a baby has a negative age");
            assert_eq!(
                game.mobs
                    .iter()
                    .filter(|m| m.kind.id == "minecraft:cow")
                    .count(),
                2,
                "only the parents are cows: no cow baby"
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
            1,
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
fn removing_the_baby_is_the_same_outcome_as_a_veto() {
    with_baby(
        |_, event| {
            event
                .edits
                .remove(event.edits.children().unwrap(), 0)
                .unwrap();
            Flow::Continue
        },
        |game| {
            let (parent, partner, child) = breed_pair(game, "minecraft:cow");
            assert_eq!(child, None);
            assert!(game.mobs.iter().all(|m| m.extra.age >= 0));
            for id in [parent, partner] {
                assert_eq!(find(game, id).extra.age, 6000);
            }
            assert_eq!(find(game, partner).extra.in_love, 0);
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

pub(crate) fn with_splits(
    listeners: Vec<SplitFn>,
    run: impl FnOnce(&mut sunrise_gameplay::game::ServerGame<'_>),
) {
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

#[test]
#[ignore = "daily"]
fn a_token_kept_from_one_listener_is_expired_in_the_next() {
    type Kept = (
        mod_api::PendingChildrenRef,
        mod_api::UnspawnedMobRef,
        mod_api::UnattachedMob,
    );
    let kept: Arc<Mutex<Option<Kept>>> = Arc::default();
    let results = Arc::new(Mutex::new(Vec::new()));
    let (first, second, out) = (kept.clone(), kept.clone(), results.clone());
    with_splits(
        vec![
            Box::new(move |_, event| {
                let edits = event.edits;
                let children = edits.children().unwrap();
                let child = edits.child(children, 0).unwrap();
                let token = edits
                    .construct(
                        children,
                        spec(
                            "minecraft:slime",
                            [1.0, 64.0, 1.0],
                            0.0,
                            Some(2),
                            event.dimension,
                        ),
                    )
                    .unwrap();
                *first.lock().unwrap() = Some((children, child, token));
                Flow::Continue
            }),
            Box::new(move |host, event| {
                let edits = event.edits;
                let (old_children, old_child, old_token) = second.lock().unwrap().unwrap();
                let fresh = edits.children().unwrap();
                *out.lock().unwrap() = vec![
                    edits.len(old_children).map(|_| ()),
                    edits.set_yaw(old_child, 1.0),
                    edits.append(fresh, old_token),
                    old_child.kind(&*host).map(|_| ()),
                    edits.len(fresh).map(|_| ()),
                ];
                Flow::Continue
            }),
        ],
        |game| {
            let mut dying = cube(game, "minecraft:slime", 4, SEED);
            game.split_cube_children(&mut dying);
            // Nothing the first listener kept reached the list: the vanilla three.
            assert_eq!(game.mobs.len(), 3);
            assert_eq!(game.mobs[0].yaw, YAWS[0]);
        },
    );
    assert_eq!(
        *results.lock().unwrap(),
        [
            Err(UnspawnedMobError::Expired),
            Err(UnspawnedMobError::Expired),
            Err(UnspawnedMobError::Expired),
            Err(UnspawnedMobError::Expired),
            Ok(()),
        ]
    );
}

pub(crate) fn animals_bred(game: &sunrise_gameplay::game::ServerGame<'_>) -> i32 {
    game.stats.get("custom", "minecraft:animals_bred")
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
            event
                .edits
                .remove(event.edits.children().unwrap(), 0)
                .unwrap();
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
fn discarding_a_parent_in_the_birth_listener_refuses_the_baby() {
    if run_fresh_cases(
        "game::creatures::pending_birth_tests::discarding_a_parent_in_the_birth_listener_refuses_the_baby",
        "SUNRISE_BIRTH_DISCARD_CHILD",
        &["discard"],
    ) {
        return;
    }
    with_baby(
        |host, event| {
            let parent = host.entity(event.parent).unwrap();
            assert!(parent.discard(host));
            assert!(host.entity(event.parent).is_none());
            Flow::Continue
        },
        |game| {
            let (parent, partner, child) = breed_pair(game, "minecraft:cow");
            assert_eq!(child, None);
            assert!(take_hook_panics().is_empty());
            assert_eq!(
                game.mobs.iter().map(|mob| mob.id).collect::<Vec<_>>(),
                [parent, partner]
            );
            assert!(
                game.entity(parent).is_some(),
                "the stale refusal rolled back the discard"
            );
            assert!(game.orbs.is_empty());
        },
    );
}

#[derive(Default)]
pub(in crate::game) struct DiscardParentOnPigJoin(pub(in crate::game) AtomicU32);

#[mod_api::__subscriptions]
impl Hooks for DiscardParentOnPigJoin {
    fn baby_entity_spawn(
        &self,
        host: &mut dyn ServerHost,
        event: &mut BabyEntitySpawnEvent<'_>,
    ) -> Flow {
        self.0.store(event.parent, Ordering::SeqCst);
        assert!(host.commands().run_command("summon minecraft:pig 4 64 4"));
        assert!(host.entity(event.parent).is_some(), "the join is deferred");
        Flow::Continue
    }

    fn entity_join_level(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::EntityJoinLevelEvent<'_>,
    ) -> Flow {
        if event.kind == "minecraft:pig" {
            let parent = self.0.load(Ordering::SeqCst);
            // This fixture reserves the two parent IDs and then the original baby ID.
            assert!(
                host.entity(parent + 2).is_some(),
                "apply precedes join publication"
            );
            assert!(host.entity(parent).unwrap().discard(host));
        }
        Flow::Continue
    }
}

#[test]
#[ignore = "daily"]
fn a_deferred_join_can_remove_a_parent_only_after_the_baby_is_applied() {
    if run_fresh_cases(
        "game::creatures::pending_birth_tests::a_deferred_join_can_remove_a_parent_only_after_the_baby_is_applied",
        "SUNRISE_BIRTH_JOIN_CHILD",
        &["join"],
    ) {
        return;
    }
    let mods = vec![mod_api::ModEntry::new(
        "parent-on-join",
        Box::new(DiscardParentOnPigJoin::default()),
    )];
    mod_platform::testing::with_mods(mods.into(), || {
        sunrise_gameplay::game::native_guest_turn_tests::with_guests(|game| {
            game.build_registries();
            let (parent, _, child) = breed_pair(game, "minecraft:cow");
            assert_eq!(child, Some(parent + 2));
            assert!(take_hook_panics().is_empty());
            assert!(
                game.entity(parent).is_none(),
                "the deferred join did remove the parent"
            );
            assert!(find(game, parent + 2).extra.age < 0);
        });
    });
}

use sunrise_gameplay_common::game::chat_common::ChatCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
