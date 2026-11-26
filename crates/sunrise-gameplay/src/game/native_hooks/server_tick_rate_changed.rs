

/// The hook points. Each does nothing in Sunrise.
impl crate::game::ServerGame<'_> {
    /// `MinecraftServer.onTickRateChanged`, before the autosave wait is adjusted: `/tick rate`
    /// set a rate or a `/tick sprint` ended.
    pub(in crate::game) fn native_server_tick_rate_changed(&mut self) {
        sunrise_gameplay_common::game::level_control_hooks::LevelControl::TickRate
            .hear(|| self.fire_server_tick_rate_changed());
    }
}