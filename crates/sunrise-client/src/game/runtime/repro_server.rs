use super::Game;
use std::path::PathBuf;

impl Game<'_, crate::game::server_side::ServerState> {
    #[inline(always)]
    #[allow(dead_code)]
    pub(in crate::game) fn start_recording_world_root_server(&self) -> Option<PathBuf> {
        if !self.side.world_root.as_os_str().is_empty() {
            Some(self.side.world_root.clone())
        } else {
            None
        }
    }
}
