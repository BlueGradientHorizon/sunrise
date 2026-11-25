use super::*;

thread_local! {
    pub(in crate::game) static PROJECT_VIEW: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
pub(in crate::game) struct ProjectionFrame(pub(in crate::game) usize);
impl Drop for ProjectionFrame {
    fn drop(&mut self) {
        PROJECT_VIEW.set(self.0);
    }
}

/// Helpers for native mods, kept as stable as the hook points.
impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// Collision boxes in block-local coordinates with an empty collision context.
    pub(in crate::game) fn block_collision(&self, pos: [i32; 3]) -> Vec<[f64; 6]> {
        self.native_block_collision_in(&self.world, pos)
    }
}
