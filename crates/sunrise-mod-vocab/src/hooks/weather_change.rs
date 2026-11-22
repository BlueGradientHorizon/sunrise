//! The `weather_change` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0341

pub trait Hooks {
    /// The server is about to set its weather parameters. `raining` and
    /// `thundering` are the requested final states.
    ///
    /// Cancellation is accepted only on normal return. A panic is logged and
    /// shown in an error toast; it disables only this mod's weather callback for
    /// the session. A phase refusal is logged each time without disabling the
    /// callback. If a listener triggers another `weather_change` while it is
    /// running, only that active callback is skipped in the nested dispatch.
    /// None of these failures undoes Host writes.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_weather_change",
        dispatch = "flat",
        side = "server",
        contract = "cancel",
        out = "cancel",
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
