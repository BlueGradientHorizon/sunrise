//! The `world_saved` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0344

pub trait Hooks {
    /// The server completed a world save cycle. `wait` says chunk writes were
    /// flushed before returning. This includes the integrated server's initial
    /// save before `server_starting`; the final save comes after player removal.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_world_saved",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        neoforge = "none (NeoForge has no such event)",
        version = 2
    )]
    fn world_saved(wait: bool);
}
