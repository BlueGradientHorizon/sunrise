//! The `add_server_reload_listeners` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register server data reload listeners after packs are selected. The
    /// [`ReloadContext`] gives registry access at registration, as
    /// `AddServerReloadListenersEvent.getRegistryAccess` does. Runs every data
    /// load on the owning server thread; world handles, services and
    /// transactions are not present on this context. Existing data-load
    /// callbacks retain their [`DataLoadContext`].
    /// Registration errors are returned and reported at the call. A panicking registration
    /// callback contributes no listeners or dependencies; unrelated callbacks still run.
    #[hook(
        receiver = "associated",
        context = "reload",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        staged_surface = "listeners",
        neoforge = "AddServerReloadListenersEvent",
        version = 4
    )]
    fn add_server_reload_listeners(listeners: &mut dyn ServerReloadListeners);
}
