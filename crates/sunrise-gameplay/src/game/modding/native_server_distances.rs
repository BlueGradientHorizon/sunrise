//! What a server mod reads of the tick times and of whether the server is published, and sets
//! of the view and simulation distance.
use crate::game::ServerGame;

impl ServerGame<'_> {
    /// `MinecraftServer.getTickTimesNanos`.
    pub(in crate::game) fn tick_times(&self) -> Vec<u64> {
        self.common.clock.tick_times()
    }
    /// `PlayerList.getViewDistance`.
    pub(in crate::game) fn server_view_distance(&self) -> Option<i32> {
        Some(self.server.lan.as_ref()?.hub.config.view_distance)
    }
    /// `PlayerList.getSimulationDistance`.
    pub(in crate::game) fn server_simulation_distance(&self) -> Option<i32> {
        Some(self.server.lan.as_ref()?.hub.config.simulation_distance)
    }
    /// `MinecraftServer.isPublished`: `DedicatedServer`'s is always true, `IntegratedServer`'s
    /// while it is open to LAN (`multiplayerScope != OFF`).
    pub(in crate::game) fn server_is_published(&self) -> bool {
        self.common.is_dedicated_server() || self.published_port().is_some()
    }
    /// `PlayerList.setViewDistance` on a dedicated server; the tickets read the settings.
    pub(in crate::game) fn server_set_view_distance(&mut self, distance: i32) -> bool {
        if !(2..=32).contains(&distance)
            || self.common.options.dedicated.is_none()
            || self.server.lan.is_none()
        {
            return false;
        }
        self.set_view_distance(distance);
        self.server.settings.render_distance = distance;
        true
    }
    /// `PlayerList.setSimulationDistance` on a dedicated server.
    pub(in crate::game) fn server_set_simulation_distance(&mut self, distance: i32) -> bool {
        if !(2..=32).contains(&distance)
            || self.common.options.dedicated.is_none()
            || self.server.lan.is_none()
        {
            return false;
        }
        self.set_simulation_distance(distance);
        self.server.settings.simulation_distance = distance;
        true
    }
}
