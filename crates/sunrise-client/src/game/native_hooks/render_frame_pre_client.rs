#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::render_frame_pre::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    pub(in crate::game::client_side) fn frame_world_partial_tick(&self) -> f64 {
        self.client
            .extras
            .frame_partial_ticks
            .map_or_else(|| self.clock.fraction(), |(world, _)| f64::from(world))
    }
}
