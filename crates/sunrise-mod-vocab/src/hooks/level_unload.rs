//! The `level_unload` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0139

pub trait Hooks {
    /// `dimension`'s level closed (NeoForge's `LevelEvent.Unload`, posted after
    /// `level.close()` in `MinecraftServer.stopServer`): drop what the mod kept
    /// for it. Fires once per level that previously fired `level_load`, either
    /// when a non-Overworld dimension unloads after nobody has been in it for
    /// 300 ticks (after its chunks' unloads) or during `MinecraftServer.stopServer`
    /// after the final save attempt and network teardown, before releasing
    /// storage and firing `server_stopped` (including after a failed open, tick
    /// crash, or failed save). Don't read its blocks. Server-side.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_level_unload",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        neoforge = "LevelEvent.Unload",
        cancellable = false,
        version = 2
    )]
    fn level_unload(dimension: &str);
}
