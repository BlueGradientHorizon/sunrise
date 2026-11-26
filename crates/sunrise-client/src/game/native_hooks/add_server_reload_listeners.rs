use super::*;

#[inline(always)]
pub(in crate::game) fn native_add_server_reload_listeners(
    context: &dyn mod_api::ReloadContext,
    register: impl FnMut(&mut dyn FnMut(&mut dyn mod_api::ServerReloadListeners) -> bool),
) {
    crate::game::native_hooks::fire::fire_add_server_reload_listeners(context, register);
}

/// The hook points. Each does nothing in Sunrise.
impl Game<'_, crate::game::server_side::ServerState> {
    /// Register server data reload listeners after packs are selected. The
    /// host gives registry access at registration, as
    /// `AddServerReloadListenersEvent.getRegistryAccess` does.
    pub(in crate::game) fn native_add_server_reload_listeners(
        &self,
        listeners: &mut mod_api::ServerListenerRegistry,
    ) {
        native_add_server_reload_listeners(self, |listen| listeners.register_listener(listen));
    }
}
