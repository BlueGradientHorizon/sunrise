//! The `potential_spawns` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Possible natural spawns at a server position (NeoForge's
    /// `LevelEvent.PotentialSpawns`, `NaturalSpawner.mobsAt`). Mods may add or
    /// remove entries, or cancel to make the list empty. Mods can read the world,
    /// but cannot write it during the spawn pass.
    #[hook(
        target = "sunrise_gameplay::game::native_hooks::native_potential_spawns::ServerGame<'_>::native_potential_spawns",
        receiver = "read",
        dispatch = "flat",
        side = "server",
        contract = "cancel",
        out = "canceled",
        host = "read",
        neoforge = "LevelEvent.PotentialSpawns",
        cancellable = true
    )]
    fn potential_spawns(
        dimension: &str,
        category: &str,
        pos: [i32; 3],
        entries: &mut Vec<SpawnEntry>,
        canceled: &mut bool,
    );
}
