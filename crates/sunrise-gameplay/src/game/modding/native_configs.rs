impl crate::game::ServerGame<'_> {
    /// The server stops: its `Server` configs unload.
    pub fn native_configs_server_stop(&mut self) {
        for file in self.common.native_configs.drop_server() {
            self.native_config_unloading(&file);
        }
    }
}
