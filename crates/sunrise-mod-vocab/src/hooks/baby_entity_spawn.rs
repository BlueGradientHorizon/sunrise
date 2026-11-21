//! The `baby_entity_spawn` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Two animals bred and the baby they made (if any) is about to join the world
    /// (NeoForge's `BabyEntitySpawnEvent`, vanilla's `Animal.spawnChildFromBreeding`
    /// and `Fox.FoxBreedGoal.breed`): `dimension` is the level they are in
    /// (a replacement baby's `PendingMobSpec` names it), `parent` and `partner` are entity
    /// ids, `player` and `player_entity_id` the UUID and entity id of the
    /// player who fed them into love (`getCausedByPlayer`, None when none).
    /// When breeding constructed a baby (a mule for a horse and a donkey), it already
    /// has its inheritance drawn, but is not finalized or in any level: it sits in
    /// the single optional breeding slot `child` (`BabyEntitySpawnChild<'_>`). If
    /// breeding produced no initial child, the hook still fires with an empty slot
    /// that a listener may populate.
    /// Inspect the slot (`event.child.get()`), edit the baby in place
    /// (`event.child.get_mut()`), construct a replacement baby
    /// (`event.child.construct(spec)?`) and swap it in (`event.child.replace(baby)?`),
    /// or remove it (`event.child.take()` / `event.child.clear()`). Any detached
    /// [`UnspawnedMob`] not left in the slot is cleanly discarded when dropped.
    /// Replacements must be baby `AgeableMob` species (and must be a baby `minecraft:fox`
    /// for fox breeding, matching NeoForge's `Fox.FoxBreedGoal.breed`).
    /// After the hook returns without cancel and with a child in the slot, breeding
    /// applies baby state, snaps the child to the live initiating parent's position
    /// with zero yaw and pitch (so listener position/yaw edits are overwritten by
    /// breeding's post-event snap), grants fox breeder trust when breeding foxes,
    /// finalizes breeding, and inserts the child once.
    /// Set `cancel` (or return `Flow::Cancel`) to abort breeding: matching NeoForge's
    /// `Animal.spawnChildFromBreeding` and `Fox.FoxBreedGoal.breed`, both parents are
    /// reset to age 6000 and in-love 0, breeding credit is cleared, and there is no
    /// baby, no experience, no `animals_bred` statistic, no advancement and no heart
    /// particles. By contrast, returning `Flow::Continue` with an empty slot skips
    /// spawning and finalization without resetting the parents or consuming credit.
    /// The inheritance draws the construction made stay spent.
    /// Not fired for turtles, frogs and sniffers (an egg, no baby), or their hatching;
    /// armadillos take the ordinary animal path and are included. Villagers do not post
    /// this animal-only event, matching NeoForge's documented `Animal.spawnChildFromBreeding`
    /// posting site; a rejected villager join instead leaves the child's bed free. On the server.
    #[notes = r#"## Baby entity spawn

- **`baby_entity_spawn(dimension, parent, partner, player, player_entity_id, child, cancel)`** (NeoForge's `BabyEntitySpawnEvent`, version 3): fires after breeding has attempted to construct the baby (`Animal.spawnChildFromBreeding` / `Fox.FoxBreedGoal.breed`) and before breeding snaps the baby, finalizes breeding, and inserts it. Even when no initial baby was constructed, the hook fires with an empty slot. `child` (`BabyEntitySpawnChild<'_>`) lends the single optional breeding slot directly on the stack: inspect the current baby with `event.child.get()`, edit it in place with `event.child.get_mut()`, construct a new baby with `event.child.construct(PendingMobSpec::new(kind, dimension, position, yaw, true))?` and swap it in with `event.child.replace(baby)?`, or clear the slot with `event.child.take()` / `event.child.clear()`. Any unattached `UnspawnedMob` value is cleaned up on normal Rust drop with no arena or sweep; replacements must be baby `AgeableMob` species (or a baby `minecraft:fox` for fox breeding), and invalid replacements answer `Invalid`.
- Cancelling (`Flow::Cancel`) resets both `parent` and `partner` to age 6000 and love 0, clears breeding credit, and spawns no baby, experience, `animals_bred` statistic, advancement or hearts. Returning `Flow::Continue` with an empty slot spawns no baby and skips breeding finalization without resetting the parents or consuming credit (matching NeoForge's `Animal.spawnChildFromBreeding` and `Fox.FoxBreedGoal.breed`). The inheritance draws already made stay spent.
- When accepted with a child, breeding applies baby state, snaps the selected child to the live initiating parent's position with zero yaw and pitch (matching vanilla's `setBaby(true)` and `snapTo(parent.x, parent.y, parent.z, 0.0, 0.0)`), grants fox breeder trust for fox breeding, runs breeding finalization, and inserts the child once.
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
