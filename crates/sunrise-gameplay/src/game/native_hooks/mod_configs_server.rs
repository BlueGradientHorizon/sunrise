use sunrise_gameplay_common::game::native_configs::ConfigFile;

impl crate::game::ServerGame<'_> {
    /// A `Server` config is dropped as its server stops
    /// (`ModConfigEvent.Unloading`), after `native_server_stopping`; the next
    /// world reads it again. `Client` and `Common` configs stay until the
    /// game quits. API 397.
    pub fn native_config_unloading(&mut self, config: &ConfigFile) {
        self.fire_config_unloading(config);
    }
}
