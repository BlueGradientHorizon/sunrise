//! Answers from the one live move, for the unedited server travel comparison.
use super::super::MotionWorld;
use mod_api::HookId;
use sunrise_core::{geometry::Vec3, position::BlockPos};
use sunrise_world::registry::BlockStateId;

#[derive(Clone)]
pub(in crate::motion) struct Answers<K, V>(Vec<(K, V)>);

impl<K, V> Default for Answers<K, V> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<K: Eq, V: Copy> Answers<K, V> {
    pub(in crate::motion) fn record(&mut self, key: K, answer: V) {
        self.0.push((key, answer));
    }

    pub(in crate::motion) fn replay(&mut self, key: K) -> Option<V> {
        let index = self.0.iter().position(|(old, _)| *old == key)?;
        Some(self.0.remove(index).1)
    }
}

#[derive(Clone, Default)]
pub(in crate::motion) struct TravelRules {
    pub edge: Answers<bool, bool>,
    pub ladder: Answers<(BlockPos, BlockStateId, bool), bool>,
    pub trapdoor: Answers<(BlockPos, BlockStateId, BlockStateId, bool), bool>,
    pub friction: Answers<(BlockPos, BlockStateId, u32), f32>,
    pub speed: Answers<u32, f32>,
    pub flying_speed: Answers<u32, f32>,
    pub speed_factor: Answers<(BlockStateId, u32), f32>,
    pub restitution: Answers<(BlockPos, BlockStateId, u32), f32>,
    pub bounce: Answers<(BlockPos, BlockStateId, u64, [u64; 3]), Vec3>,
    pub movement: Answers<[u64; 3], Option<Vec3>>,
    pub fluid_shapes: Option<sunrise_world::fluid_shape::FluidShapeCache>,
    pub missing: bool,
}

/// A missing block answer matters only when a Part could have answered it.
pub(in crate::motion) fn part_may_answer(hook: HookId, state: BlockStateId) -> bool {
    mod_api::__private::scoped_state_listens(hook, state.0)
        .unwrap_or_else(|| mod_api::__private::has_scoped_hook(hook))
}

impl MotionWorld<'_, '_, '_> {
    pub(in crate::motion) fn travel_rule<K: Eq, V: Copy>(
        &mut self,
        answers: fn(&mut TravelRules) -> &mut Answers<K, V>,
        question: K,
        vanilla: V,
        may_listen: impl FnOnce() -> bool,
        live: impl FnOnce(&mut Self) -> V,
    ) -> V {
        if self.player.travel_probe {
            let rules = self.player.travel_rules.as_mut().unwrap();
            return answers(rules).replay(question).unwrap_or_else(|| {
                rules.missing |= may_listen();
                vanilla
            });
        }
        let value = live(self);
        if let Some(rules) = &mut self.player.travel_rules {
            answers(rules).record(question, value);
        }
        value
    }
}
