use super::Game;
use mod_api::Authority;

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn client_lifecycle_open_started(&mut self) {
        self.client_reset_entity_residency();
        self.entity_initialization = Default::default();
        if let Some(remote) = &mut self.remote {
            remote.clear_attachments();
        }
        self.attachment_indexes.clear();
        self.passages.retire_world();
        self.native_lifecycle.begin_open(Authority::RemoteClient);
    }

    pub(in crate::game) fn client_lifecycle_world_ready(&mut self) {
        let _ = self.native_lifecycle.world_open_ready();
    }

    /// Client epochs retire without dispatching server lifecycle hooks.
    pub(in crate::game) fn client_lifecycle_world_closed(&mut self) {
        self.side.instance_groups.borrow_mut().clear();
        let _ = self.native_lifecycle.world_close_final();
        self.finish_world_closed_common();
    }
}
