//! The `level_load` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0138

pub trait Hooks {
    /// `dimension`'s level opened on the server (NeoForge's `LevelEvent.Load`,
    /// posted in `MinecraftServer.createLevels`): before any of its chunks
    /// (`chunk_load`), so a mod sets up what it keeps for the level
    /// here. At startup the Overworld fires first, before its initial world
    /// spawn is chosen; the remaining levels follow the configured
    /// `LEVEL_STEM` registry traversal. Do not assume any particular order
    /// among them. Each fires before the next level is available. Levels stay open while idle (only their chunks and
    /// entities evict), so idle levels do not fire `level_unload` or a second
    /// `level_load` on revisit.
    /// Explicitly opened runtime levels can fire this while parked:
    /// `event.dimension` names the opened level, not the Host's current level.
    /// Server-side.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_level_load",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        neoforge = "LevelEvent.Load",
        cancellable = false,
        version = 3
    )]
    fn level_load(dimension: &str);
}
