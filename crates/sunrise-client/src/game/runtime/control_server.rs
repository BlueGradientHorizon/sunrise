use super::Game;
use sunrise_client::Result;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `sunrise-ctl save` and `save-all`.
    pub(in crate::game) fn control_save(&mut self, all: bool) -> Result<()> {
        if all {
            let keys: Vec<_> = self.world.chunks.keys().copied().collect();
            self.world.modified.extend(keys);
        }
        self.save_game()
    }
}
