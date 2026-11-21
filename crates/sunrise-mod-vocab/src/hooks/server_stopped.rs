//! The `server_stopped` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0008

pub trait Hooks {
    /// Fired once at the end of server teardown (NeoForge's
    /// `ServerStoppedEvent` via `ServerLifecycleHooks.handleServerStopped`),
    /// after `stopServer` completes (player cleanup, final save attempt for an
    /// adopted world, network teardown, `level_unload` for opened levels, and
    /// storage release), on both integrated and dedicated servers. Always fires
    /// once startup has begun, even when startup failed, the tick loop errored
    /// or panicked, the final save failed, or a shutdown listener panicked.
    /// Failed opens save only worlds already adopted by the server (the
    /// prepared level is installed; the session lock alone is not adoption).
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_server_stopped",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        neoforge = "ServerStoppedEvent",
        cancellable = false
    )]
    fn server_stopped(dedicated: bool);
}
