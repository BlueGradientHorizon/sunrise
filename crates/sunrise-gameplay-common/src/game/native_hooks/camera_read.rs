use super::*;

thread_local! {
    pub(in crate::game) static PROJECT_VIEW: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
pub struct ProjectionFrame(pub(in crate::game) usize);
impl Drop for ProjectionFrame {
    fn drop(&mut self) {
        PROJECT_VIEW.set(self.0);
    }
}

/// Helpers for native mods, kept as stable as the hook points.
impl crate::game::CommonGame<'_> {
    /// Collision boxes in block-local coordinates with an empty collision context.
    pub fn block_collision(&self, pos: [i32; 3]) -> Vec<[f64; 6]> {
        self.native_block_collision_in(&self.world, pos)
    }
}
