use super::*;

impl Game<'_, crate::game::server_side::ServerState> {
    #[inline(always)]
    #[allow(dead_code)]
    pub(in crate::game) fn init_remote_for_replay_server(&mut self) {
        self.side.save.0 = Arc::new(SaveDir::new(
            std::env::temp_dir().join("sunrise-replay-unused"),
            sunrise_client::save::LevelShape::OVERWORLD,
            None,
        ));
    }
    #[inline(always)]
    #[allow(dead_code)]
    pub(in crate::game) fn install_remote_session_server(&mut self) {
        self.side.save.0 = Arc::new(SaveDir::new(
            std::env::temp_dir().join("sunrise-remote-unused"),
            sunrise_client::save::LevelShape::OVERWORLD,
            None,
        ));
    }
}
