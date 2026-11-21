//! The `mob_spawn_checks` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// NeoForge's `MobSpawnEvent.SpawnPlacementCheck`: may the entity type `kind`
    /// spawn at `pos` in `level` for `reason`? This server hook runs after
    /// `SpawnPlacements.checkSpawnRules` evaluates the type's spawn predicate.
    /// `default_result` is the effective predicate result, including predicates composed
    /// or replaced through `Registrar::register_spawn_placement`. It can differ from
    /// vanilla even for a vanilla entity type. `result` starts as `Default`, which keeps
    /// that answer. Set `Succeed` or `Fail` to override it for this check. The predicates
    /// have already made their live random draws; overriding the result doesn't undo them.
    ///
    /// It runs wherever the game calls `checkSpawnRules` with the game's level: natural
    /// spawning, village sieges, patrols, zombie reinforcements, spawners and trial spawners.
    /// Ordinary spawners with custom spawn rules skip it; trial spawners check their
    /// custom rules afterward. `/summon` and spawn eggs do not check placements.
    /// `level` (also named by `dimension`) is the level the mob would spawn in. `spawner`
    /// is `Spawner::Block` or `Spawner::Trial` if a spawner block asked, or `None` otherwise,
    /// including a spawner minecart. The hook lends a read-only host. Listeners should be
    /// pure functions of their inputs because the checks run again on replayed ticks.
    ///
    /// Beyond NeoForge: one `SpawnCheck` answers this hook and `mob_spawn_position_check`;
    /// NeoForge gives them separate enums with the same three constants.
    #[notes = r#"## Spawn checks

- **`mob_spawn_placement_check(dimension, kind, reason, pos, default_result, level, spawner, result)`** (NeoForge's `MobSpawnEvent.SpawnPlacementCheck`) and **`mob_spawn_position_check(dimension, kind, reason, pos, level, spawner, mob, result)`** (`MobSpawnEvent.PositionCheck`, version 2): set `result` to `SpawnCheck::Succeed` or `Fail` to force a spawn check; `Default` keeps the game's answer. In placement hook version 2, `default_result` is the effective predicate result, including any predicates composed or replaced through `Registrar::register_spawn_placement`; it can differ from vanilla even for a vanilla entity type. `Default` keeps that result. The predicates have already made their live random draws, and overriding their result neither skips those draws nor rolls them back. The position check stands in for `Mob.checkSpawnRules` and `checkSpawnObstruction`: with no listener or `Default` they run and draw as vanilla, while `Succeed` and `Fail` skip them and their draws. On the game's level, the placement check runs for natural spawning, spawners, trial spawners, patrols, sieges and reinforcements. An ordinary spawner with custom rules skips it; trial spawners check their custom rules afterward. The position check also runs for chunk generation, and only replaces obstruction for trial spawners. Both get `level` (`LevelRef`, the level the mob would join) and `spawner` (`Option<Spawner>`, `#[non_exhaustive]`: `Block { pos }` for a spawner block, `Trial { pos }` for a trial spawner, `None` where no block asked: natural spawning, chunk generation, patrols, sieges, reinforcements and a spawner minecart). The position check also gets `mob` (`UnspawnedMobView<'_>`), a scoped read-only borrow of the mob itself: vanilla creates the mob before this check, and Sunrise, which otherwise creates it after, creates it before the listeners only when the hook is listened, the way the site creates it, and hands that same mob on to the spawn, so the mob a listener read is the one that spawns; a refused check drops it, never added to the level, and gives its provisional id back. Creating it draws nothing from the level's random, so a listener that answers `Default` leaves every draw as an unlistened spawn's. Read it directly on `event.mob` without `host` (`kind`, `position`, `equipment`, `attribute_base`, `attribute_value`, `tags`, `custom_name`); there is no editor and no `Expired` handle, as the borrow cannot outlive the call. Where the site does not create the mob fresh, the view answers `kind` and `position` and `Unsupported` for the rest: chunk generation's packs (the generator creates and finalizes them with its own random, after the check) and a spawner or trial spawner whose spawn data carries entity data (the game loads it from that data after the tick). Both get the read-only host and run on replayed ticks, so a listener is a pure function of its inputs. Tick-time effect: one `listened` branch with no listeners, before any `SpawnReason` is built; with a position-check listener, the mob is created one step early, not twice. One `SpawnCheck` serves both (NeoForge's two enums have the same constants)."#]
    #[hook(
        receiver = "read",
        dispatch = "api_carrier",
        side = "server",
        contract = "listen",
        host = "read",
        neoforge = "MobSpawnEvent.SpawnPlacementCheck",
        cancellable = false,
        version = 2
    )]
    fn mob_spawn_placement_check(
        dimension: &str,
        kind: &str,
        reason: SpawnReason,
        pos: BlockPos,
        default_result: bool,
        level: LevelRef,
        spawner: Option<Spawner>,
        result: &mut SpawnCheck,
    );
    /// NeoForge's `MobSpawnEvent.PositionCheck`: may the freshly created mob of type `kind` stand at `pos` (its position, a mob's centre) in `level` for `reason`? Vanilla's two checks are `Mob.checkSpawnRules` (the type's own rules) and `Mob.checkSpawnObstruction` (inside blocks or fluids, an ocelot's grass and sea level). `result` starts as `Default`, which runs them; `Succeed` or `Fail` answers instead and they are skipped. It runs on natural spawning, chunk generation's creature packs and spawner blocks (where a spawner with custom spawn rules skips `Mob.checkSpawnRules` on `Default`, as vanilla does).  A trial spawner runs only `checkSpawnObstruction`, so only that is skipped. `spawner` is the block that asked (`Spawner::Block` or `Spawner::Trial`), `None` for natural spawning, chunk generation and a spawner minecart. `mob` is a scoped read-only [`UnspawnedMobView`] of the mob itself, in no level yet: read it directly without `host` (`event.mob.kind()`, `event.mob.position()`, `event.mob.equipment(slot)`). It is the mob that spawns if the check passes, handed on rather than created again, and dropped without trace if it fails. Where the game does not create the mob fresh (chunk generation's packs, a spawner whose spawn data carries entity data) the view answers its `kind` and `position` and `Unsupported` for the rest. Server-side. With no listener or `Default` the checks and their random draws are vanilla's (creating the mob for the listener draws nothing from the level's random); `Succeed` and `Fail` skip the checks and so their draws, and the level random's order changes only then; a listener should be a pure function of its inputs, because ticks are replayed.
    #[hook(
        receiver = "read",
        dispatch = "api_carrier",
        side = "server",
        contract = "listen",
        host = "read",
        neoforge = "MobSpawnEvent.PositionCheck",
        cancellable = false,
        version = 2
    )]
    fn mob_spawn_position_check(
        dimension: &str,
        kind: &str,
        reason: SpawnReason,
        pos: [f64; 3],
        level: LevelRef,
        spawner: Option<Spawner>,
        mob: UnspawnedMobView<'_>,
        result: &mut SpawnCheck,
    );
}
