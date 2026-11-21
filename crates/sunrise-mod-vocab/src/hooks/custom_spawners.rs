//! The custom spawner hook: declaration the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// NeoForge's `ModifyCustomSpawnersEvent`: the server level `dimension` builds its custom
    /// spawner list once when the level opens. `spawners` starts with vanilla's five built-ins
    /// (`WanderingTrader`, `Patrol`, `Cat`, `VillageSiege` and `Phantom`, in the order Sunrise
    /// ticks them) for the Overworld, and starts empty for other dimensions (matching vanilla
    /// `MinecraftServer#createLevels`). Take a built-in spawner out with `remove` and it never
    /// ticks, so its random draws are gone with it; append an owned `CustomSpawner` object with
    /// `add` and the level owns that instance for its lifetime, ticking it directly once per
    /// eligible server tick (`spawn_mobs` enabled) inside a server transaction and dropping it
    /// when the level unloads or the world closes. With no listener, or one that leaves the list
    /// alone, the level ticks exactly vanilla's spawners. The phantom spawner runs after the
    /// level's mobs have ticked, as the game runs it, wherever it is in the list. Server-side,
    /// not cancellable.
    #[notes = r#"## Custom spawners

`modify_custom_spawners` (NeoForge's `ModifyCustomSpawnersEvent`) fires once per server level when it opens, with the level's `CustomSpawners` list: vanilla's five built-ins (`WanderingTrader`, `Patrol`, `Cat`, `VillageSiege`, `Phantom`) in the order Sunrise ticks them for the Overworld, and an empty list for other dimensions (`MinecraftServer#createLevels`). `remove` takes a built-in out so it never ticks (its random draws go with it); `add` appends an owned `CustomSpawner` instance (a struct implementing `CustomSpawner` or a closure) to the end of the list. With no listener, or one that leaves the list alone, nothing changes: the same spawners tick in the same order.

Each registered `CustomSpawner` is owned by that level for its lifetime, ticked directly as its registering mod once per eligible server tick (`spawn_mobs` enabled) inside a server transaction with `spawn_enemies` matching `ServerLevel#isSpawningMonsters()` (`spawn_monsters` enabled and difficulty not peaceful), and dropped when the level unloads or the world closes. A phase refusal rolls back that tick's writes without disabling the spawner; a panic rolls back that tick's writes, disables that spawner type for the registering mod for the rest of the session, logs loudly and notifies the client error toast queue.

Sunrise ticks vanilla's spawners where it always has: the trader, patrol, cat and siege spawners in the custom spawner block (vanilla's own order differs, and moving it is a separate parity change) and the phantom spawner with the stats, after the mobs. A mod's spawners tick after them in list order, so the position of the `Phantom` entry in the list does not move it."#]
    #[hook(
        version = 2,
        receiver = "read",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        host = "read",
        neoforge = "ModifyCustomSpawnersEvent",
        cancellable = false
    )]
    fn modify_custom_spawners(dimension: &str, spawners: &mut mod_api::CustomSpawners);
}
