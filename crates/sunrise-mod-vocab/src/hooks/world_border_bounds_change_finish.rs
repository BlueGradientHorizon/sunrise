//! The `world_border_bounds_change_finish` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0343

pub trait Hooks {
    /// After a moving border on the server reaches its target size on `level`.
    /// Starting or replacing an extent does not finish it. A panicking mod
    /// callback is logged, shown in an error toast and disabled for the session.
    /// Nested dispatch skips callbacks already running on this thread.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_world_border_bounds_change_finish",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        version = 2,
        isolate_listener = true,
        neoforge = "none (NeoForge has no such event; Bukkit's WorldBorderBoundsChangeFinish)",
        cancellable = false
    )]
    fn world_border_bounds_change_finish(level: LevelRef);
}
