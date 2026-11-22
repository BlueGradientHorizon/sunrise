//! The `world_border_bounds_changed` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0273

pub trait Hooks {
    /// After the server sets `level`'s border size. `lerp_ticks` is `None` for a fixed
    /// size and `Some(requested_ticks)` for a timed write, including a timed equal-size
    /// write. Does not run for loads or client packets. A panicking mod callback is
    /// logged, shown in an error toast and disabled for the session. Nested dispatch
    /// skips callbacks already running on this thread.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_world_border_bounds_changed",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        version = 2,
        isolate_listener = true,
        neoforge = "none (NeoForge has no such event)"
    )]
    fn world_border_bounds_changed(level: LevelRef, lerp_ticks: Option<i64>);
}
