//! Common mod pack-finder discovery hook shared across server and client roles.
use super::Game;
use mod_api::{HookId, PackType};
use sunrise_client::packs::Pack;

impl Game<'_> {
    pub(in crate::game) fn discover_mod_packs(&mut self, kind: PackType) -> Vec<Pack> {
        if !mod_api::__private::enabled_listened(HookId::AddPackFinders) {
            return Vec::new();
        }
        let mut registry = super::pack_finders::Registry::new(kind, self.mods.clone());
        self.native_add_pack_finders(&mut registry);
        registry.packs
    }
}
