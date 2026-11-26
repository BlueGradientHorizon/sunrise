use crate::game::ClientGame;

impl ClientGame<'_> {
    /// The window's frame step: changed config files reload once a second.
    pub(in crate::game) fn native_configs_tick_client(&mut self) {
        self.common.native_configs_poll();
    }
}
