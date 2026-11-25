use super::*;
use sunrise_items::creative_catalog::{Catalog, Holders};

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn server_creative_holders(&self) -> Option<Holders> {
        self.registry_sync().map(|sync| sync.creative_holders())
    }

    pub(in crate::game) fn server_creative_catalog(&self) -> &Arc<Catalog> {
        &self.side.creative_catalog.0
    }

    #[cfg(test)]
    pub(in crate::game) fn test_set_creative_catalog(&mut self, catalog: Arc<Catalog>) {
        self.side.creative_catalog.0 = catalog;
    }
}

#[cfg(test)]
#[path = "build_creative_mode_tab_contents/tests_server.rs"]
pub(in crate::game) mod tests;
