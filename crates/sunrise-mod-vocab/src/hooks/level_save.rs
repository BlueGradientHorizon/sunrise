//! The `level_save` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// `dimension`'s level was saved (NeoForge's `LevelEvent.Save`, in
    /// `ServerLevel.save`): its level data, chunks and entities, by the
    /// autosave, `/save-all`, a pause, a closing level or the server
    /// stopping, and the integrated server's initial save before
    /// `server_starting`. A world-wide save visits levels in their creation
    /// order, regardless of which level is active. Idle chunk eviction is
    /// not a level save. Server-side.
    #[hook(
        dispatch = "flat",
        side = "server",
        contract = "listen",
        neoforge = "LevelEvent.Save",
        paper = "WorldSaveEvent",
        version = 2
    )]
    fn level_save(dimension: &str);
}
