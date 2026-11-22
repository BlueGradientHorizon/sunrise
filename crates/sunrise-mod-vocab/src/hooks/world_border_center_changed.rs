//! The `world_border_center_changed` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0274

pub trait Hooks {
    /// After the server sets `level`'s border center, including an equal-value direct write.
    /// Does not run for loads or client packets. A panicking mod callback is logged, shown in
    /// an error toast and disabled for the session. Nested dispatch skips callbacks already
    /// running on this thread.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_world_border_center_changed",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        version = 2,
        isolate_listener = true,
        neoforge = "none (NeoForge has no such event)"
    )]
    fn world_border_center_changed(level: LevelRef);
}
