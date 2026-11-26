//! The `world_save_start` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// The server is starting a world save cycle, before anything is written: before the
    /// levels' chunks, the players and the world's metadata (level.dat). What a listener changes
    /// here goes out in this same cycle: block entities it marks changed, saved data and player
    /// data it updates. On the stop's final save the players have already been written as the
    /// player list closed, so there it comes before the levels and level.dat only. `wait` is the flag `world_saved` reports at the cycle's end: true when
    /// this save flushes its chunk writes before returning (`/save-all flush`, a pause, the
    /// final save on stop), false for the autosave. Once for every cycle, paired with one
    /// `world_saved` unless the cycle fails; a cycle the server refuses to start (a transfer
    /// still settling) asks neither. Not asked on a client of a remote server.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_world_save_start",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        neoforge = "none (NeoForge has no such event; LevelEvent.Save fires per level, after the levels before it were saved)"
    )]
    fn world_save_start(wait: bool);
}
