//! The `add_server_reload_listeners` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register server data reload listeners after packs are selected. The
    /// host gives registry access at registration, as
    /// `AddServerReloadListenersEvent.getRegistryAccess` does. Runs every data
    /// load on the owning server thread in Reloading: only registry facts,
    /// settings and mod-list reads are available through this Host. World
    /// handles, services and transactions panic naming the phase. Existing
    /// data-load callbacks retain their DataLoadContext; no setup event is added.
    /// Dependencies on the function library order both its prepare and admission;
    /// every listener prepares before any apply.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        host = "read",
        neoforge = "AddServerReloadListenersEvent",
        version = 3
    )]
    fn add_server_reload_listeners(listeners: &mut dyn ServerReloadListeners);
}
