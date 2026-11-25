#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::api_values::block_state::block_listens;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::animate_tick::LentRandom;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::animate_tick::LevelRandom;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::animate_tick::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::cell::RefCell;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::rc::Rc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::LegacyRandom;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Lend `ClientLevel.random` to a client Part, sharing the stream with any callbacks
    /// and host effects it calls before returning.
    pub(in crate::game) fn lend_client_level_random<R>(
        &mut self,
        f: impl FnOnce(&mut Self, &mut dyn mod_api::RandomSource) -> R,
    ) -> R {
        if let Some(lent) = self.side.lent_level_random.0.clone() {
            return f(self, &mut LevelRandom(lent));
        }
        let lent: LentRandom = Rc::new(RefCell::new(mod_api::LegacyRandomSource::from(<(
            u64,
            Option<f64>,
        )>::from(
            &*self.side.particles.0.level_random(),
        ))));
        self.side.lent_level_random.0 = Some(lent.clone());
        let out = f(self, &mut LevelRandom(lent.clone()));
        self.side.lent_level_random.0 = None;
        *self.side.particles.0.level_random() =
            LegacyRandom::from(<(u64, Option<f64>)>::from(&*lent.borrow()));
        out
    }
    /// Runs `f`, which draws from the level random (breaking-particle position, `add_particle`'s limiter or
    /// the poofs from `create_fireworks`), on the stream a client Part
    /// holds as `event.random`: the engine's random is that stream for `f`, and
    /// `f`'s draws stay in it.
    pub(in crate::game::client_side) fn with_lent_level_random<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let Some(lent) = self.side.lent_level_random.0.clone() else {
            return f(self);
        };
        *self.side.particles.0.level_random() =
            LegacyRandom::from(<(u64, Option<f64>)>::from(&*lent.borrow()));
        let out = f(self);
        *lent.borrow_mut() = mod_api::LegacyRandomSource::from(<(u64, Option<f64>)>::from(
            &*self.side.particles.0.level_random(),
        ));
        out
    }
}
