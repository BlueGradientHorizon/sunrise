//! `Block.animateTick` for mod blocks: the parts of the blocks `ClientLevel.animateTick` picked.
use super::*;
use crate::game::api_values::block_state::block_listens;
use std::cell::RefCell;
use sunrise_core::random::LegacyRandom;

/// One client tick's picks (`ClientLevel.animateTick`'s sampled positions) whose block has a
/// part answering `animate_tick`, in pick order: the sampler records them as it goes, and the
/// game calls the parts once it ends.
#[derive(Default)]
pub(in crate::game) struct AnimatePicks(RefCell<Vec<([i32; 3], BlockStateId)>>);

impl AnimatePicks {
    /// A picked position: kept when its state's block has a part answering `animate_tick`
    /// (the per-block table's answer, no dispatch).
    pub(in crate::game) fn record(&self, state: BlockStateId, pos: [i32; 3]) {
        if block_listens(mod_api::HookId::AnimateTick, state) {
            self.0.borrow_mut().push((pos, state));
        }
    }
}

impl Game<'_> {
    /// The picks a client tick's sampling records for the block parts, or none (nothing to
    /// record) while no part answers `animate_tick` or off a window's client.
    pub(in crate::game) fn mod_animate_picks(&self) -> Option<AnimatePicks> {
        (mod_api::__private::has_scoped_hook(mod_api::HookId::AnimateTick) && self.native_window())
            .then(AnimatePicks::default)
    }

    /// Calls the parts of each recorded pick, in pick order, with the client's level and its
    /// random (`ClientLevel.random`): the parts' draws move it as vanilla's `animateTick` draws do.
    pub(in crate::game) fn mod_animate_ticks(&mut self, picks: AnimatePicks) {
        let picks = picks.0.into_inner();
        if picks.is_empty() {
            return;
        }
        let Some(level) = self.native_client_level() else {
            return;
        };
        let mut random = mod_api::LegacyRandomSource::from(<(u64, Option<f64>)>::from(
            &*self.particles.level_random(),
        ));
        for (pos, state) in picks {
            self.native_animate_tick(super::fire::AnimateTickCall {
                level,
                pos: BlockPos::new(pos[0], pos[1], pos[2]),
                state,
                random: &mut random,
            });
        }
        *self.particles.level_random() = LegacyRandom::from(<(u64, Option<f64>)>::from(&random));
    }

    /// A client's random display tick of the mod block `state` at `pos` (`Block.animateTick`),
    /// picked by `ClientLevel.animateTick`'s sampling: its parts add particles and play sounds
    /// through the client host, drawing from the level's `random`. Client-side.
    pub(in crate::game) fn native_animate_tick(&mut self, call: super::fire::AnimateTickCall<'_>) {
        self.fire_animate_tick(call);
    }
}
