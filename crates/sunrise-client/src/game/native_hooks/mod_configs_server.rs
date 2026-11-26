use super::*;

use crate::game::native_configs::ConfigFile;

impl Game<'_, crate::game::server_side::ServerState> {
    /// A `Server` config is dropped as its server stops
    /// (`ModConfigEvent.Unloading`), after `native_server_stopping`; the next
    /// world reads it again. `Client` and `Common` configs stay until the
    /// game quits. API 397.
    pub(in crate::game) fn native_config_unloading(&mut self, config: &ConfigFile) {
        self.fire_config_unloading(config);
    }
}
