use super::*;
use sunrise_items::creative_catalog::{Catalog, Holders};

impl crate::game::ServerGame<'_> {
    pub(in crate::game) fn server_creative_holders(&self) -> Option<Holders> {
        self.registry_sync().map(|sync| sync.creative_holders())
    }

    pub(in crate::game) fn server_creative_catalog(&self) -> &Arc<Catalog> {
        &self.server.creative_catalog.0
    }

    #[cfg(test)]
    pub(in crate::game) fn test_set_creative_catalog(&mut self, catalog: Arc<Catalog>) {
        self.server.creative_catalog.0 = catalog;
    }
}
