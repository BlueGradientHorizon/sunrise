//! The `baby_entity_spawn` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Two animals bred and the baby they made is about to join the world
    /// (NeoForge's `BabyEntitySpawnEvent`, vanilla's
    /// `Animal.spawnChildFromBreeding`): `dimension` is the level they are in
    /// (a replacement baby's `PendingMobSpec` names it), `parent` and `partner` are entity
    /// ids, `player` and `player_entity_id` the UUID and entity id of the
    /// player who fed them into love (`getCausedByPlayer`, None when none).
    /// The baby (a mule for a horse and a donkey) is already constructed,
    /// with its inheritance drawn, but not finalized or in any level: it sits in
    /// the single optional breeding slot `child` (`BabyEntitySpawnChild<'_>`).
    /// Inspect it (`event.child.get()`), edit it in place (`event.child.get_mut()`),
    /// construct a replacement baby (`event.child.construct(spec)?`) and swap it in
    /// (`event.child.replace(baby)?`), or remove it (`event.child.take()` /
    /// `event.child.clear()`) for none. Any detached [`UnspawnedMob`] not left in the slot
    /// is cleanly discarded when dropped, and a replacement must be a baby.
    /// Set `cancel` (or return `Flow::Cancel`) for no baby either: matching NeoForge's
    /// `Animal.spawnChildFromBreeding`, both parents are reset to age 6000 and in-love 0
    /// and there is no baby, no experience, no `animals_bred` statistic, no advancement
    /// and no heart particles. The inheritance draws the construction made stay spent.
    /// Not fired for turtles, frogs and sniffers (an egg, no baby), or their hatching;
    /// armadillos take the ordinary animal path and are included. Villagers do not post
    /// this animal-only event, matching NeoForge's documented `Animal.spawnChildFromBreeding`
    /// posting site; a rejected villager join instead leaves the child's bed free. On the server.
    #[notes = r#"## Baby entity spawn

- **`baby_entity_spawn(dimension, parent, partner, player, player_entity_id, child, cancel)`** (NeoForge's `BabyEntitySpawnEvent`, version 3): fires after breeding has constructed the baby (`Animal.spawnChildFromBreeding`: `getBreedOffspring`, baby flag and position) and before breeding is finalized and the baby inserted. `child` (`BabyEntitySpawnChild<'_>`) lends the single optional breeding slot directly on the stack: inspect the current baby with `event.child.get()`, edit it in place with `event.child.get_mut()`, construct a new baby with `event.child.construct(PendingMobSpec::new(kind, dimension, position, yaw, true))?` and swap it in with `event.child.replace(baby)?`, or clear the slot with `event.child.take()` / `event.child.clear()`. Any unattached `UnspawnedMob` value is cleaned up on normal Rust drop with no arena or sweep, and a non-baby replacement answers `Invalid`.
- Cancelling (`Flow::Cancel`), or emptying the slot, breeds no child: matching NeoForge's `Animal.spawnChildFromBreeding`, both `parent` and `partner` are reset to age 6000 and love 0, and nothing awards experience, the `animals_bred` statistic, an advancement or hearts. The inheritance draws already made stay spent. A replacement is inserted exactly once at the original insertion point, and breeding's own finalization (credit, statistics, experience, the parent's cooldown) reads it as the child.
- Turtle and frog breeding, sniffer egg laying and turtle hatching make eggs, not babies, and do not fire it. Armadillos breed through the ordinary animal path and do. Villagers breed through their own behavior and do not (a separate moment)."#]
    #[hook(
        dispatch = "api_carrier",
        side = "server",
        version = 3,
        contract = "cancel",
        out = "cancel",
        neoforge = "BabyEntitySpawnEvent",
        cancellable = true
    )]
    fn baby_entity_spawn(
        dimension: &str,
        parent: u32,
        partner: u32,
        player: Option<u128>,
        player_entity_id: Option<i32>,
        child: BabyEntitySpawnChild<'_>,
        cancel: &mut bool,
    );
}
