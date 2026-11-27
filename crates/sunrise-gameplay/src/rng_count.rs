//! Per-phase level-random draw attribution and capture-counter publishing (`--features rng-count`).

#[cfg(feature = "rng-count")]
pub use sunrise_core::random::draw_count::{Phase, PhaseGuard, enter as guard, in_phase};

#[cfg(not(feature = "rng-count"))]
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    RandomBlockTicks,
    ScheduledTicks,
    NaturalSpawning,
    MobBatch,
    Weather,
    BlockEntities,
    Raids,
    PlayerActions,
    Other,
}

#[cfg(not(feature = "rng-count"))]
pub struct PhaseGuard;

#[cfg(not(feature = "rng-count"))]
#[inline(always)]
pub fn guard(_phase: Phase) -> PhaseGuard {
    PhaseGuard
}

#[cfg(not(feature = "rng-count"))]
#[inline(always)]
pub fn in_phase<T>(_phase: Phase, f: impl FnOnce() -> T) -> T {
    f()
}

#[cfg(feature = "rng-count")]
pub fn publish_tick() {
    use std::sync::OnceLock;
    use sunrise_perf_format::Counter;

    static COUNTERS: OnceLock<[Counter; Phase::COUNT]> = OnceLock::new();
    let counters = COUNTERS.get_or_init(|| {
        let registry = sunrise_perf_format::counter_registry();
        Phase::ALL.map(|phase| {
            registry.counter_with_unit("level_random.draws", &[("phase", phase.name())], "draws")
        })
    });
    let counts = sunrise_core::random::draw_count::take();
    for (counter, count) in counters.iter().zip(counts) {
        if count != 0 {
            counter.add(count);
        }
    }
}

#[cfg(not(feature = "rng-count"))]
#[inline(always)]
pub fn publish_tick() {}

#[cfg(all(test, feature = "rng-count"))]
mod tests {
    use super::*;
    use sunrise_core::random::{EntityRandom, LegacyRandom, RandomSource};
    use sunrise_level::world::World;
    use sunrise_perf_format::{CounterReporter, Interner, counter_registry, v1::Side};
    use sunrise_sim::health::{DamageHooks, DamageLevel, LentLevel};
    use sunrise_world::registry::{BlockRegistry, BlockStateId};

    #[test]
    fn lent_world_random_attributes_to_mob_batch_and_publishes_capture_counters() {
        let registry = counter_registry();
        let before: [u64; Phase::COUNT] = Phase::ALL.map(|phase| {
            registry
                .counter_with_unit("level_random.draws", &[("phase", phase.name())], "draws")
                .get()
        });
        sunrise_core::random::draw_count::reset();

        let facts =
            br#"{"defaults":{"minecraft:air":0},"states":[["air",true,false,"","minecraft:air"]]}"#;
        let blocks = BlockRegistry::from_facts_json(&facts[..]).unwrap();
        let mut world = World::new(BlockStateId(0), 42);
        let mut entity_rng = EntityRandom::own(LegacyRandom::new(7));

        in_phase(Phase::Weather, || {
            let _ = world.random.next_int();
        });
        in_phase(Phase::RandomBlockTicks, || {
            let _ = world.random.next_float();
            let _ = world.random.next_boolean();
            let _ = entity_rng.next_int();
        });
        in_phase(Phase::MobBatch, || {
            let mut lent = LentLevel::take(&mut world);
            {
                let mut damage_level =
                    DamageLevel::lent(&world, &blocks, &mut lent, None, None, &DamageHooks::NONE);
                let _ = damage_level.random().next_int();
                let _ = damage_level.random().next_double();
            }
            // Placeholder installed in `world.random` while lent is not the level source.
            let _ = world.random.next_long();
            lent.give_back(&mut world);
        });
        in_phase(Phase::PlayerActions, || {
            let _ = world.random.next_int();
        });
        let _ = world.random.next_float();

        publish_tick();

        let delta = |phase: Phase| {
            registry
                .counter("level_random.draws", &[("phase", phase.name())])
                .get()
                - before[phase as usize]
        };
        assert_eq!(delta(Phase::Weather), 1);
        assert_eq!(delta(Phase::RandomBlockTicks), 2);
        assert_eq!(delta(Phase::MobBatch), 3);
        assert_eq!(delta(Phase::PlayerActions), 1);
        assert_eq!(delta(Phase::Other), 1);
        assert_eq!(delta(Phase::ScheduledTicks), 0);
        assert_eq!(delta(Phase::NaturalSpawning), 0);
        assert_eq!(delta(Phase::BlockEntities), 0);
        assert_eq!(delta(Phase::Raids), 0);

        let mut strings = Interner::default();
        let mut reporter = CounterReporter::new(Side::Server);
        let report = reporter.report(registry, &mut strings, 1.0);
        let entries = strings.take_new(Side::Server);
        let text = |id: u32| {
            entries
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| entry.text.as_str())
        };
        let mob_def = report
            .defs
            .iter()
            .find(|def| {
                text(def.name) == Some("level_random.draws")
                    && def
                        .labels
                        .iter()
                        .any(|l| text(l.key) == Some("phase") && text(l.value) == Some("mob_batch"))
            })
            .expect("level_random.draws{phase=mob_batch} counter definition");
        assert_eq!(text(mob_def.unit), Some("draws"));
    }
}
