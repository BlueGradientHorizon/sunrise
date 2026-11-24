#[allow(unused_imports)]
use super::*;

#[cfg(test)]
#[path = "boss_darken_tests.rs"]
pub(in crate::game) mod boss_darken_tests;

/// `ClientClockManager.tick` for one clock: it moves on by `delta` game ticks at its rate, the
/// part of a tick it makes carried to the next. None, and the clock left as it was, when its total
/// would pass the range of a long: a server's times must not wrap or panic the client.
pub(in crate::game) fn advance_clock(clock: &mut play::ClockState, delta: i64) -> Option<()> {
    let partial = f64::from(clock.partial_tick) + delta as f64 * f64::from(clock.rate);
    let full = partial.floor();
    if !(i64::MIN as f64..i64::MAX as f64).contains(&full) {
        return None;
    }
    clock.total_ticks = clock.total_ticks.checked_add(full as i64)?;
    clock.partial_tick = (partial - full) as f32;
    Some(())
}

/// Per-level client work bracketed by the shared LevelTickEvent hooks.
/// No level means no event; failed ticks never emit the post event.
impl Game<'_> {
    pub(in crate::game) fn tick_remote_level(&mut self) -> Result<()> {
        self.tick_remote_level_with(Self::tick_remote_level_body)
    }

    pub(in crate::game) fn tick_remote_level_with(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        let level = (mod_api::__private::listened(mod_api::HookId::LevelTickPre)
            || mod_api::__private::listened(mod_api::HookId::LevelTickPost))
        .then(|| self.native_client_level())
        .flatten();
        if let Some(level) = level {
            self.native_level_tick_pre(level);
        }
        let result = body(self);
        // Each entity's own move this tick (`EntityEvent.EnteringSection`).
        self.client_entity_sections_stepped();
        // `ClientLevel.tick`'s RETURN, which a failed tick does not reach.
        if result.is_ok()
            && let Some(level) = level
        {
            self.native_level_tick_post(level);
        }
        result
    }
}
