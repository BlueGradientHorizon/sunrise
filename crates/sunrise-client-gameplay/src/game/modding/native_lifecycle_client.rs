use crate::game::ClientGame;
use mod_api::Authority;

impl ClientGame<'_> {
    pub(in crate::game) fn client_lifecycle_open_started(&mut self) {
        self.client_reset_entity_residency();
        self.common.entity_initialization = Default::default();
        if let Some(remote) = &mut self.client.remote {
            remote.clear_attachments();
        }
        self.common.attachment_indexes.clear();
        self.common.passages.retire_world();
        self.common.native_lifecycle.begin_open(Authority::RemoteClient);
    }

    pub(in crate::game) fn client_lifecycle_world_ready(&mut self) {
        let _ = self.common.native_lifecycle.world_open_ready();
    }

    /// Client epochs retire without dispatching server lifecycle hooks.
    pub(in crate::game) fn client_lifecycle_world_closed(&mut self) {
        self.client.instance_groups.borrow_mut().clear();
        let _ = self.common.native_lifecycle.world_close_final();
        self.finish_world_closed_common();
    }
}

use sunrise_gameplay_common::game::native_lifecycle_common::NativeLifecycleCommonGameExt1 as _;
