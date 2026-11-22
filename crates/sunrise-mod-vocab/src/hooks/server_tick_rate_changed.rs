//! The `server_tick_rate_changed` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// `MinecraftServer.onTickRateChanged`, before the server adjusts its autosave wait. Runs
    /// after every successful tick-rate write, including an equal value, before the server sends
    /// the resulting clock state. Also runs when a `/tick sprint` ends: on its own, by
    /// `/tick sprint stop` or by `/tick freeze`. Read the current rate with `host.tick_stats()`.
    /// Server-only: a client receiving the rate or reporting a sprint does not fire it.
    ///
    /// Calling `host.set_tick_rate` from a listener still writes the rate; nested dispatch skips
    /// callbacks already running on this thread. A panicking callback is logged, shown in an
    /// error toast and disabled for the session. NeoForge has no counterpart.
    #[hook(
        dispatch = "flat",
        side = "server",
        contract = "listen",
        version = 2,
        isolate_listener = true,
        neoforge = "none (vanilla MinecraftServer.onTickRateChanged; NeoForge adds no event or extension for it)"
    )]
    fn server_tick_rate_changed();
}
