use super::*;

impl mod_api::PackDiscoveryContext for Game<'_> {
    fn client_text(&self) -> Option<&dyn mod_api::prelude::ClientTextClientHost> {
        if self.is_logical_server() {
            None
        } else {
            Some(self)
        }
    }
}

impl Game<'_> {
    /// Register sources when a resource or data pack repository is discovered.
    pub(in crate::game) fn native_add_pack_finders(&self, packs: &mut dyn mod_api::PackFinders) {
        Self::fire_add_pack_finders(self, packs);
    }
}
