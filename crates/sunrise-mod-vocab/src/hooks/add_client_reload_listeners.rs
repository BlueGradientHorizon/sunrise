//! The `add_client_reload_listeners` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register client asset reload listeners before a reload stages its
    /// assets (NeoForge's `AddClientReloadListenersEvent`, the client's
    /// counterpart of `add_server_reload_listeners`): the first load
    /// on the loader thread, then every later reload — the player
    /// changed resource packs, F3+T was pressed, or a server sent its packs.
    /// The listener's `prepare` then reads the merged asset view with
    /// resource-pack overrides resolved, and its returned apply publishes the
    /// parsed state, all applies after all prepares, before
    /// `client_resources_reloaded`. Dependencies on the model manager order
    /// both its real model build and its publication. Atlas upload precedes the model manager. A listener prepare or engine preflight that refuses the
    /// candidate aborts the load before any listener applies.
    /// Client-side, in Reloading nested in Setup initially and World later.
    /// Later registration is on the render thread, prepare on the loader,
    /// and apply on the owning game thread; completion restores the entering
    /// phase. Startup's empty Host grants no extra access. Registration errors
    /// are returned and logged at the call, without stopping the reload. A
    /// registration callback that panics or incurs a phase refusal contributes
    /// no listeners or dependencies; unrelated callbacks still run.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        staged_surface = "listeners",
        version = 4,
        neoforge = "AddClientReloadListenersEvent"
    )]
    fn add_client_reload_listeners(listeners: &mut dyn ClientReloadListeners);
}
