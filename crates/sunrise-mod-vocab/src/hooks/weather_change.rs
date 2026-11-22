//! The `weather_change` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0341

pub trait Hooks {
    /// Before `/weather` or a mod sets the server-wide weather parameters.
    /// `dimension` is the origin of the write, not its scope. `clear_ticks`
    /// is the requested clear timer and `weather_ticks` sets both the rain
    /// and thunder timers. These timers and the `raining` and `thundering`
    /// flags are read-only proposals: editing your event copy changes neither
    /// the write nor what other listeners see. Natural weather ticks and
    /// direct strength changes do not fire this hook.
    ///
    /// Return `Flow::Cancel` or set `event.cancelled` to skip the proposed write.
    /// A veto is accepted only on normal return and is logged with the mod's ID.
    /// A panic is logged and shown in an error toast; it disables only this
    /// mod's weather callback for the session. A phase refusal is logged each
    /// time without disabling the callback. Nested dispatch skips callbacks
    /// already running on this thread; other eligible listeners still run.
    /// A veto or failure does not undo Host writes.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_weather_change",
        dispatch = "flat",
        side = "server",
        contract = "cancel",
        out = "cancel",
        version = 2,
        isolate_listener = true,
        neoforge = "none (NeoForge has no such event)",
        cancellable = true
    )]
    fn weather_change(
        dimension: &str,
        clear_ticks: i32,
        weather_ticks: i32,
        raining: bool,
        thundering: bool,
        cancel: &mut bool,
    );
}
