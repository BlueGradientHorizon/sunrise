use super::*;

macro_rules! impl_pack_discovery_context {
    ($game:ty) => {
        impl mod_api::PackDiscoveryContext for $game {
            fn live_host(&mut self) -> Option<mod_api::HostSide<'_>> {
                Some(super::super::execution_role::host_side(self))
            }

            fn client_text(&self) -> Option<&dyn mod_api::prelude::ClientTextClientHost> {
                crate::game::Side::as_client_text_host(self)
            }
        }
    };
}

impl_pack_discovery_context!(crate::game::ServerGame<'_>);
impl_pack_discovery_context!(crate::game::ClientGame<'_>);

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// Register sources when a resource or data pack repository is discovered.
    pub(in crate::game) fn native_add_pack_finders(
        &mut self,
        packs: &mut crate::game::pack_finders::Registry,
    ) {
        let mods = self.mods.clone();
        crate::game::native_hooks::fire::fire_add_pack_finders(
            crate::game::Side::as_pack_discovery_context_mut(self),
            |listen| packs.register_listener(listen),
            |id| mods.code_owner_enabled(id),
        );
    }
}
