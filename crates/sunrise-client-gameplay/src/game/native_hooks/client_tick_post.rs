/// The hook points. Each does nothing in Sunrise.
#[cfg(feature = "client")]
impl crate::game::ClientGame<'_> {
    /// Once a client tick, after the window's tick ran (`ClientTickEvent.Post`).
    pub(in crate::game) fn native_client_tick_post(&mut self) {
        self.fire_client_tick_post();
    }
}
