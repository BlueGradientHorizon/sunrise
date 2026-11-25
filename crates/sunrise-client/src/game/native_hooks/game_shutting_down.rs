use super::*;

/// The hook points. Each does nothing in Sunrise.
impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The process is about to exit after its game has stopped.
    pub(in crate::game) fn native_game_shutting_down(&mut self) {
        self.fire_game_shutting_down();
    }
}
