use super::*;

/// The hook points. Each does nothing in Sunrise.
impl crate::game::CommonGame<'_> {
    /// The process is about to exit after its game has stopped.
    pub fn native_game_shutting_down(&mut self) {
        self.fire_game_shutting_down();
    }
}
