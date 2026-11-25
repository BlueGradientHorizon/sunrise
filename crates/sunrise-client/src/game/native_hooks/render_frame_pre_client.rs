use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game::client_side) fn frame_world_partial_tick(&self) -> f64 {
        self.side
            .extras
            .frame_partial_ticks
            .map_or_else(|| self.clock.fraction(), |(world, _)| f64::from(world))
    }
}
