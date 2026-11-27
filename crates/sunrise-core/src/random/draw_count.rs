//! Per-phase LCG draw counter for the level-wide random source (`LevelRandom` / `World.random`).
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Phase {
    RandomBlockTicks = 0,
    ScheduledTicks = 1,
    NaturalSpawning = 2,
    MobBatch = 3,
    Weather = 4,
    BlockEntities = 5,
    Raids = 6,
    PlayerActions = 7,
    Other = 8,
}

impl Phase {
    pub const COUNT: usize = 9;
    pub const ALL: [Self; Self::COUNT] = [
        Self::RandomBlockTicks,
        Self::ScheduledTicks,
        Self::NaturalSpawning,
        Self::MobBatch,
        Self::Weather,
        Self::BlockEntities,
        Self::Raids,
        Self::PlayerActions,
        Self::Other,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::RandomBlockTicks => "random_block_ticks",
            Self::ScheduledTicks => "scheduled_ticks",
            Self::NaturalSpawning => "natural_spawning",
            Self::MobBatch => "mob_batch",
            Self::Weather => "weather",
            Self::BlockEntities => "block_entities",
            Self::Raids => "raids",
            Self::PlayerActions => "player_actions",
            Self::Other => "other",
        }
    }
}

thread_local! {
    static PHASE: Cell<Phase> = const { Cell::new(Phase::Other) };
    static COUNTS: [Cell<u64>; Phase::COUNT] = const { [const { Cell::new(0) }; Phase::COUNT] };
}

#[must_use = "the random-draw phase ends when the guard is dropped"]
pub struct PhaseGuard(Phase);

impl Drop for PhaseGuard {
    #[inline]
    fn drop(&mut self) {
        PHASE.with(|current| current.set(self.0));
    }
}

#[inline]
pub fn enter(phase: Phase) -> PhaseGuard {
    PhaseGuard(PHASE.with(|current| current.replace(phase)))
}

#[inline]
pub fn in_phase<T>(phase: Phase, f: impl FnOnce() -> T) -> T {
    let _guard = enter(phase);
    f()
}

#[inline]
pub fn current() -> Phase {
    PHASE.with(Cell::get)
}

#[inline]
pub(super) fn record() {
    let phase = PHASE.with(Cell::get) as usize;
    COUNTS.with(|counts| {
        let slot = &counts[phase];
        slot.set(slot.get().wrapping_add(1));
    });
}

pub fn take() -> [u64; Phase::COUNT] {
    COUNTS.with(|counts| std::array::from_fn(|i| counts[i].replace(0)))
}

pub fn reset() {
    let _ = take();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::BlockPos;
    use crate::random::{
        EntityRandom, InvalidBound, LegacyRandom, LevelRandom, PositionalRandom, RandomSource,
    };

    #[test]
    fn attributes_level_draws_by_phase_and_ignores_other_sources() {
        reset();
        let mut level = LevelRandom::for_level(LegacyRandom::new(1));
        let mut entity = EntityRandom::own(LegacyRandom::new(2));
        let mut plain = LegacyRandom::new(3);

        let _ = level.next_int();
        let _ = entity.next_int();
        let _ = plain.next_long();

        in_phase(Phase::RandomBlockTicks, || {
            let _ = level.next_float();
            let _ = level.next_boolean();
            let _ = entity.next_double();
        });

        {
            let _outer = enter(Phase::MobBatch);
            let _ = level.next_int();
            {
                let _inner = enter(Phase::Weather);
                let _ = level.next_double();
            }
            assert_eq!(current(), Phase::MobBatch);
            let _ = level.next_float();
        }
        assert_eq!(current(), Phase::Other);

        // `fork` draws its seed (next_long = 2 LCG steps) from the level source;
        // draws on the forked source itself are not level-wide draws.
        let mut forked = in_phase(Phase::Raids, || level.fork());
        in_phase(Phase::Raids, || {
            let _ = forked.next_int();
            let _ = forked.next_long();
        });
        let pos_rng = in_phase(Phase::ScheduledTicks, || level.fork_positional());
        let mut at = pos_rng.at(BlockPos::new(1, 2, 3));
        in_phase(Phase::ScheduledTicks, || {
            let _ = at.next_int();
        });

        let counts = take();
        assert_eq!(counts[Phase::Other as usize], 1);
        assert_eq!(counts[Phase::RandomBlockTicks as usize], 2);
        assert_eq!(counts[Phase::MobBatch as usize], 2);
        assert_eq!(counts[Phase::Weather as usize], 2);
        assert_eq!(counts[Phase::Raids as usize], 2);
        assert_eq!(counts[Phase::ScheduledTicks as usize], 2);
        assert_eq!(counts[Phase::NaturalSpawning as usize], 0);
        assert_eq!(counts[Phase::BlockEntities as usize], 0);
        assert_eq!(counts[Phase::PlayerActions as usize], 0);
        assert_eq!(take(), [0; Phase::COUNT]);
    }

    #[test]
    fn lent_level_random_counts_while_swapped_out_and_after_return() {
        reset();
        let mut level = LevelRandom::for_level(LegacyRandom::new(99));

        // Matches `LentLevel::take`: swap the inner `LegacyRandom` out for a placeholder.
        let mut lent = std::mem::replace(&mut *level, LegacyRandom::new(0));
        in_phase(Phase::MobBatch, || {
            let _ = lent.next_int();
            let _ = lent.next_long();
        });
        // Drawing from the temporary placeholder does not count.
        in_phase(Phase::Other, || {
            let _ = level.next_int();
            let _ = level.next_double();
        });
        *level = lent;

        in_phase(Phase::BlockEntities, || {
            let _ = level.next_boolean();
        });

        let counts = take();
        assert_eq!(counts[Phase::MobBatch as usize], 3);
        assert_eq!(counts[Phase::Other as usize], 0);
        assert_eq!(counts[Phase::BlockEntities as usize], 1);
    }

    #[test]
    fn cloned_level_random_counts_and_invalid_bound_draws_nothing() {
        reset();
        let mut level = LevelRandom::for_level(LegacyRandom::new(7));
        let mut cloned = level.clone();

        in_phase(Phase::PlayerActions, || {
            assert_eq!(level.next_int_bound(0), Err(InvalidBound));
            let _ = cloned.next_int();
            let _ = cloned.next_float();
        });

        let counts = take();
        assert_eq!(counts[Phase::PlayerActions as usize], 2);
    }
}
