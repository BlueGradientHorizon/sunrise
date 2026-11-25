#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::render_frame_post::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game::client_side) fn finish_render_frame(&mut self, elapsed: Duration) {
        let (world_partial_tick, partial_tick) = self
            .side
            .extras
            .frame_partial_ticks
            .take()
            .unwrap_or_else(|| self.frame_partial_ticks());
        self.native_render_frame_post(elapsed, world_partial_tick, partial_tick);
    }
}
