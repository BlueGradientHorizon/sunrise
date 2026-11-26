//! #795, the mob slice: a handle fired inside a mob's own tick resolves the
//! mob the tick holds out of the game, without a copy or a placeholder.
use {crate::game::creatures::borrowed_tick::*};
use mod_api::{
    Despawn, EntityTickListenedEvent, EntityTickPreEvent, HostSide, ModEntry, ServerHost,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};

/// The `mob_despawn` facts a listener reads through its handle: its own
/// position, and another mob's through a nested lookup.
#[derive(Debug, Default)]
pub(in crate::game) struct DespawnRecords(
    pub(in crate::game) Vec<(u32, Option<[f64; 3]>, Option<[f64; 3]>)>,
);

pub(in crate::game) struct DespawnProbe {
    pub(in crate::game) records: Arc<Mutex<DespawnRecords>>,
    pub(in crate::game) allow: Arc<AtomicU32>,
}

#[mod_api::__subscriptions]
impl mod_api::Hooks for DespawnProbe {
    fn mob_despawn(&self, host: &dyn mod_api::Host, event: &mut mod_api::MobDespawnEvent<'_>) {
        let id = event.mob.entity().id();
        let own = event.mob.entity().position(host);
        let other_id = if id == 71 { 72 } else { 71 };
        let other = host
            .entity(other_id)
            .and_then(|entity| entity.position(host));
        self.records.lock().unwrap().0.push((id, own, other));
        if id == self.allow.load(Ordering::Relaxed) {
            *event.result = Despawn::Allow;
        }
    }
}

/// Reads and moves the mob in its `entity_tick_pre`, through its handle.
pub(in crate::game) struct MoveOnPre {
    pub(in crate::game) id: u32,
    pub(in crate::game) seen: Arc<Mutex<Vec<Option<[f64; 3]>>>>,
}

#[mod_api::__subscriptions]
impl mod_api::Hooks for MoveOnPre {
    fn entity_tick_listened(&self, _: HostSide<'_>, event: &mut EntityTickListenedEvent) {
        event.listened = true;
    }
    fn entity_tick_pre(&self, host: HostSide<'_>, event: &mut EntityTickPreEvent<'_>) {
        let HostSide::Server(host) = host else {
            panic!("server fixture");
        };
        if event.entities.contains(&self.id) {
            let entity = host.entity(self.id).expect("a live handle in its own Pre");
            let at = entity
                .position(host)
                .expect("the tick view reads its position");
            self.seen.lock().unwrap().push(Some(at));
            assert!(
                entity.teleport_to(host, [at[0], at[1] + 10.0, at[2]], None),
                "the tick view writes its position"
            );
        }
    }
}

#[allow(unused_imports)]
pub(in crate::game) use crate::game::server_side::creatures_server::borrowed_tick::mob_tick_access_tests::with_two_pigs;

/// Removes the mob it names, and records what every despawn callback reads of
/// that mob's handle: the victim's own callback runs before the removal, a
/// later callback after it.
pub(in crate::game) struct RemovedPeerProbe {
    pub(in crate::game) victim: u32,
    pub(in crate::game) records: Arc<Mutex<Vec<(u32, Option<[f64; 3]>)>>>,
}

#[mod_api::__subscriptions]
impl mod_api::Hooks for RemovedPeerProbe {
    fn mob_despawn(&self, host: &dyn mod_api::Host, event: &mut mod_api::MobDespawnEvent<'_>) {
        let id = event.mob.entity().id();
        let victim = host
            .entity(self.victim)
            .and_then(|entity| entity.position(host));
        self.records.lock().unwrap().push((id, victim));
        if id == self.victim {
            *event.result = Despawn::Allow;
        }
    }
}

/// The view's removed state is authoritative: once the first mob is removed by
/// its own callback, a later callback asking for its handle gets nothing, even
/// though the removed row stays indexed with its old position.
#[test]
#[ignore = "daily"]
fn a_removed_mob_handle_resolves_nothing_in_a_later_callback() {
    let records = Arc::new(Mutex::new(Vec::new()));
    let mods: mod_api::Mods = vec![ModEntry::new(
        "mob-tick-removed-peer",
        Box::new(RemovedPeerProbe {
            victim: 71,
            records: records.clone(),
        }),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_two_pigs(|game| {
            game.tick_creature_entities().unwrap();
            let records = records.lock().unwrap();
            assert_eq!(records.len(), 2, "both mobs heard their despawn check");
            // 71 runs first and is removed only after its own answer.
            assert_eq!(records[0].0, 71);
            assert_eq!(
                records[0].1,
                Some([1.0, 80.0, 0.5]),
                "the victim still resolves inside its own callback"
            );
            // 72 runs after 71's removal: its row is indexed but marked removed.
            assert_eq!(records[1].0, 72);
            assert_eq!(
                records[1].1, None,
                "a removed mob's handle resolves nothing"
            );
            assert!(
                !game.mobs.iter().any(|mob| mob.id == 71),
                "the allowed mob was removed"
            );
            assert!(
                game.mobs.iter().any(|mob| mob.id == 72),
                "the denied mob stayed"
            );
        });
    });
}

/// The mob the pass holds out of the game answers its own handle, and the
/// handle of the other mob (the tick snapshot) too; `Allow` removes the
/// mob and its handle then resolves nothing.
#[test]
#[ignore = "daily"]
fn mob_despawn_handles_resolve_the_ticking_mob_and_its_peers() {
    let records = Arc::new(Mutex::new(DespawnRecords::default()));
    let allow = Arc::new(AtomicU32::new(72));
    let mods: mod_api::Mods = vec![ModEntry::new(
        "mob-tick-access",
        Box::new(DespawnProbe {
            records: records.clone(),
            allow: allow.clone(),
        }),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_two_pigs(|game| {
            game.tick_creature_entities().unwrap();
            let records = records.lock().unwrap();
            assert_eq!(records.0.len(), 2, "both mobs heard their despawn check");
            for (id, own, peer) in &records.0 {
                let (expected_own, expected_peer) = match id {
                    71 => ([1.0, 80.0, 0.5], [2.0, 80.0, 0.5]),
                    72 => ([2.0, 80.0, 0.5], [1.0, 80.0, 0.5]),
                    _ => panic!("unexpected mob {id}"),
                };
                assert_eq!(*own, Some(expected_own), "own position, mob {id}");
                assert_eq!(*peer, Some(expected_peer), "different peer, mob {id}");
            }
            // `Allow` removed 72 during the pass; 71 is still there.
            assert!(
                game.mobs.iter().any(|mob| mob.id == 71),
                "the denied mob stayed"
            );
            assert!(
                !game.mobs.iter().any(|mob| mob.id == 72),
                "the allowed mob was removed"
            );
        });
    });
}

/// A mob's own `entity_tick_pre` handle reads where it stands and moves it;
/// the real tick then starts from the position the handle wrote.
#[test]
#[ignore = "daily"]
fn entity_tick_pre_reads_and_moves_the_ticking_mob() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mods: mod_api::Mods = vec![ModEntry::new(
        "mob-tick-pre-access",
        Box::new(MoveOnPre {
            id: 71,
            seen: seen.clone(),
        }),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_two_pigs(|game| {
            game.tick_creature_entities().unwrap();
            assert_eq!(
                seen.lock().unwrap().as_slice(),
                &[Some([1.0, 80.0, 0.5])],
                "the Pre handle read the mob's position once"
            );
            let mob = game.mobs.iter().find(|mob| mob.id == 71).unwrap();
            assert_eq!(
                mob.position.y, 90.0,
                "the tick started from the position the handle wrote"
            );
        });
    });
}

/// Observations made by the actual scoped despawn hook, not by the spawn
/// fixture or a replacement host.
#[derive(Debug, PartialEq)]
pub(in crate::game) struct PassObservation {
    pub(in crate::game) actor: u32,
    pub(in crate::game) peer_velocity: Option<[f64; 3]>,
    pub(in crate::game) spawned_position: Option<[f64; 3]>,
    pub(in crate::game) spawned_handles: [bool; 3],
}

pub(in crate::game) struct PassProbe {
    pub(in crate::game) records: Arc<Mutex<Vec<PassObservation>>>,
}

#[mod_api::__subscriptions]
impl mod_api::Hooks for PassProbe {
    fn mob_despawn(&self, host: &dyn mod_api::Host, event: &mut mod_api::MobDespawnEvent<'_>) {
        let spawned = mod_api::EntityRef::new(event.mob.entity().epoch(), 74);
        self.records.lock().unwrap().push(PassObservation {
            actor: event.mob.entity().id(),
            peer_velocity: host.entity(72).and_then(|peer| peer.velocity(host)),
            spawned_position: spawned.position(host),
            spawned_handles: [
                host.entity(74).is_some(),
                host.living(spawned).is_some(),
                host.mob(spawned).is_some(),
            ],
        });
        *event.result = Despawn::Deny;
    }
}

/// NoAI still drops motion below 0.003 during its real turn. The observers
/// bracket that turn, so a frozen initial snapshot or an all-post-turn view
/// cannot satisfy both assertions. The existing lifted-pass spawn fixture
/// inserts a real mob after the pass has taken its snapshot and storage;
/// observations still go through the actual scoped hook on both passes.
#[test]
#[ignore = "daily"]
fn scoped_despawn_view_tracks_peer_turns_and_defers_mid_pass_spawns() {
    let records = Arc::new(Mutex::new(Vec::new()));
    let mods: mod_api::Mods = vec![ModEntry::new(
        "mob-tick-pass-visibility",
        Box::new(PassProbe {
            records: records.clone(),
        }),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_two_pigs(|game| {
            game.mobs[1].position.x = 4.0;
            game.mobs[1].velocity = Vec3::new(0.001, 0.0, 0.0);
            let kind = game.res.mob_types().get("minecraft:pig").unwrap().clone();
            let mut observer = Mob::new(73, kind, Vec3::new(7.0, 80.0, 0.5), 0.0);
            observer.no_ai = true;
            observer.no_gravity = true;
            observer.ground.on_ground = true;
            game.mobs.push(observer);
            crate::game::server_side::entity_server::taken_pass_tests::with_spawn(
                |game| {
                    let kind = game.res.mob_types().get("minecraft:pig").unwrap().clone();
                    let mut spawned = Mob::new(74, kind, Vec3::new(10.0, 80.0, 0.5), 0.0);
                    spawned.no_ai = true;
                    spawned.no_gravity = true;
                    spawned.ground.on_ground = true;
                    game.mobs.push(spawned);
                    // The ordinary host can already read the spawn buffer.
                    // Only the scoped view must keep it outside this pass.
                    let entity = game.entity(74).expect("spawn has a resident handle");
                    assert!(game.mob(entity).is_some());
                    assert_eq!(entity.position(game), Some([10.0, 80.0, 0.5]));
                },
                || game.tick_creature_entities().unwrap(),
            );
            assert_eq!(
                records.lock().unwrap().as_slice(),
                &[
                    PassObservation {
                        actor: 71,
                        peer_velocity: Some([0.001, 0.0, 0.0]),
                        spawned_position: None,
                        spawned_handles: [false; 3],
                    },
                    PassObservation {
                        actor: 72,
                        peer_velocity: Some([0.001, 0.0, 0.0]),
                        spawned_position: None,
                        spawned_handles: [false; 3],
                    },
                    PassObservation {
                        actor: 73,
                        peer_velocity: Some([0.0, 0.0, 0.0]),
                        spawned_position: None,
                        spawned_handles: [false; 3],
                    },
                ]
            );
            let spawned = game.mobs.iter().find(|mob| mob.id == 74).unwrap();
            assert_eq!(spawned.age, 0, "the mid-pass spawn has not taken a turn");
            records.lock().unwrap().clear();
            game.tick_creature_entities().unwrap();
            let records = records.lock().unwrap();
            assert_eq!(records.len(), 4, "the spawn joins the next pass");
            for actor in [71, 72, 73, 74] {
                let observation = records.iter().find(|record| record.actor == actor).unwrap();
                assert_eq!(observation.peer_velocity, Some([0.0, 0.0, 0.0]));
                assert_eq!(observation.spawned_position, Some([10.0, 80.0, 0.5]));
                assert_eq!(observation.spawned_handles, [true; 3]);
            }
        });
    });
}

/// Extends the lifetime of the dropped item it names, through `extra_life`.
pub(in crate::game) struct ExtendLife {
    pub(in crate::game) id: Arc<AtomicU32>,
}

#[mod_api::__subscriptions]
impl mod_api::Hooks for ExtendLife {
    fn item_expire(&self, _: &mut dyn ServerHost, event: &mut mod_api::ItemExpireEvent<'_>) {
        if event.entity == self.id.load(Ordering::Relaxed) {
            event.extra_life += 100;
        }
    }
}

/// Discards another mob from the first mob's `entity_tick_pre`: a hook that
/// re-enters the entity handles while a tick is running.
pub(in crate::game) struct DiscardOnPre {
    pub(in crate::game) id: u32,
    pub(in crate::game) victim: u32,
    pub(in crate::game) seen: Arc<Mutex<Vec<(u32, bool, bool)>>>,
}

#[mod_api::__subscriptions]
impl mod_api::Hooks for DiscardOnPre {
    fn entity_tick_listened(&self, _: HostSide<'_>, event: &mut EntityTickListenedEvent) {
        event.listened = true;
    }
    fn entity_tick_pre(&self, host: HostSide<'_>, event: &mut EntityTickPreEvent<'_>) {
        let HostSide::Server(host) = host else {
            panic!("server fixture");
        };
        if event.entities.contains(&self.id) {
            let victim = host
                .entity(self.victim)
                .expect("the other mob is live at the Pre");
            let discarded = victim.discard(host);
            let self_live = host.entity(self.id).is_some();
            self.seen
                .lock()
                .unwrap()
                .push((self.victim, discarded, self_live));
        }
    }
}

#[test]
#[ignore = "daily"]
fn a_tick_hook_discards_another_mob_nested() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mods: mod_api::Mods = vec![ModEntry::new(
        "mob-tick-nested-discard",
        Box::new(DiscardOnPre {
            id: 71,
            victim: 72,
            seen: seen.clone(),
        }),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_two_pigs(|game| {
            game.tick_creature_entities().unwrap();
            assert_eq!(
                seen.lock().unwrap().as_slice(),
                &[(72, true, true)],
                "the nested discard ran, and the actor's handle still resolved"
            );
            assert!(game.mobs.iter().any(|mob| mob.id == 71));
            assert!(!game.mobs.iter().any(|mob| mob.id == 72));
        });
    });
}

/// What a tick listener heard: each call's phase and entities, and at a Post where the first
/// of them stands. Names the types of `named` to `entity_tick_each`.
pub(in crate::game) struct TickTiming {
    pub(in crate::game) named: &'static [&'static str],
    pub(in crate::game) replace_cancels: bool,
    pub(in crate::game) heard: Arc<Mutex<TickCalls>>,
}

/// A tick listener's calls, in order: (phase, entities, the first one's position at a Post).
pub(in crate::game) type TickCalls = Vec<(&'static str, Vec<u32>, Option<[f64; 3]>)>;

#[mod_api::__subscriptions]
impl mod_api::Hooks for TickTiming {
    fn entity_tick_listened(&self, _: HostSide<'_>, event: &mut EntityTickListenedEvent) {
        event.listened = true;
    }
    fn entity_tick_pre(&self, _: HostSide<'_>, event: &mut EntityTickPreEvent<'_>) {
        let mut heard = self.heard.lock().unwrap();
        heard.push(("pre", event.entities.to_vec(), None));
        if self.replace_cancels {
            event.canceled.clear();
            if event.entities == [71] {
                event.canceled.push(71);
            }
        }
    }
    fn entity_tick_post(&self, host: HostSide<'_>, event: &mut mod_api::EntityTickPostEvent<'_>) {
        let HostSide::Server(host) = host else {
            panic!("server fixture");
        };
        let at = host
            .entity(event.entities[0])
            .and_then(|entity| entity.position(host));
        let mut heard = self.heard.lock().unwrap();
        heard.push(("post", event.entities.to_vec(), at));
    }
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        for kind in self.named {
            r.entity_tick_each(kind);
        }
    }
}

/// The two pigs' tick hooks through the real creature pass; the first pig falls in its tick.
pub(in crate::game) fn pig_ticks(named: bool, replace_cancels: bool) -> TickCalls {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let mods: mod_api::Mods = vec![ModEntry::new(
        "mob-tick-timing",
        Box::new(TickTiming {
            named: if named { &["minecraft:pig"] } else { &[] },
            replace_cancels,
            heard: heard.clone(),
        }),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_two_pigs(|game| {
            let first = game.mobs.iter().position(|mob| mob.id == 71).unwrap();
            game.mobs.update(first, |mob| {
                mob.no_ai = false;
                mob.no_gravity = false;
                mob.ground.on_ground = false;
                // Already falling: a mob moves by its velocity before gravity adds to it, so
                // one at rest would not move in its first tick.
                mob.velocity = Vec3::new(0.0, -0.5, 0.0);
            });
            game.tick_creature_entities().unwrap();
        });
    });
    std::mem::take(&mut *heard.lock().unwrap())
}

/// A type a mod names is told at NeoForge's moment: each pig alone, its Post right after its
/// own tick and before the next pig's Pre, and a Post handle reads where its tick took it.
#[test]
#[ignore = "daily"]
fn a_named_type_hears_each_entitys_own_pre_and_post() {
    let heard = pig_ticks(true, false);
    let phases: Vec<_> = heard
        .iter()
        .map(|(phase, entities, _)| (*phase, entities.clone()))
        .collect();
    assert_eq!(
        phases,
        [
            ("pre", vec![71]),
            ("post", vec![71]),
            ("pre", vec![72]),
            ("post", vec![72]),
        ]
    );
    let first = heard[1].2.expect("the Post reads the first pig");
    assert!(first[1] < 80.0, "the first pig fell in its tick: {first:?}");
    assert_eq!(heard[3].2, Some([2.0, 80.0, 0.5]), "the second pig stayed");
}

#[test]
#[ignore = "daily"]
fn a_named_pre_cannot_clear_an_earlier_turns_cancellation() {
    let phases: Vec<_> = pig_ticks(true, true)
        .into_iter()
        .map(|(phase, entities, _)| (phase, entities))
        .collect();
    assert_eq!(
        phases,
        [("pre", vec![71]), ("pre", vec![72]), ("post", vec![72])]
    );
}

/// With no type named, the mobs are told as one family: one Pre before the first tick, one
/// Post after the last, as before.
#[test]
#[ignore = "daily"]
fn an_unnamed_family_hears_one_pre_and_one_post() {
    let phases: Vec<_> = pig_ticks(false, false)
        .into_iter()
        .map(|(phase, entities, _)| (phase, entities))
        .collect();
    assert_eq!(phases, [("pre", vec![71, 72]), ("post", vec![71, 72])]);
}

/// Moves pig 72 into a second arrow's path in the Post of the arrow `first` (an arrow named to
/// `entity_tick_each`, so told alone).
pub(in crate::game) struct MoveOnArrowPost {
    pub(in crate::game) first: Arc<AtomicU32>,
}

#[mod_api::__subscriptions]
impl mod_api::Hooks for MoveOnArrowPost {
    fn entity_tick_listened(&self, _: HostSide<'_>, event: &mut EntityTickListenedEvent) {
        event.listened = true;
    }
    fn entity_tick_post(&self, host: HostSide<'_>, event: &mut mod_api::EntityTickPostEvent<'_>) {
        let HostSide::Server(host) = host else {
            panic!("server fixture");
        };
        if event.entities == [self.first.load(Ordering::Relaxed)] {
            let pig = host.entity(72).expect("the pig");
            assert!(pig.teleport_to(host, [4.0, 80.0, 5.5], None));
        }
    }
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        r.entity_tick_each("minecraft:arrow");
    }
}

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt6 as _;
