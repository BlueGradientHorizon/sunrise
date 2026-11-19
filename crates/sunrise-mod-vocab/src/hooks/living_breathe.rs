//! The `living_breathe` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0151

pub trait Hooks {
    /// An entity's breathing this tick (`LivingBreatheEvent`, posted from
    /// `LivingEntity.baseTick` every tick for a living entity with the generic
    /// air logic): `entity` is its ID and `kind` its type (`minecraft:player`,
    /// `minecraft:zombie`). `air` is `[current, maximum]` before the update.
    /// Current air can be negative; the maximum is 300 for players and most
    /// mobs, 4800 for dolphins and 6000 for axolotls. Every listener gets the
    /// same pair; writing to it has no effect. Only `can_breathe`, `consume`
    /// and `refill` carry forward between listeners and change this tick.
    /// With `can_breathe` the air rises by `refill`, up to its maximum; a
    /// dolphin's vanilla refill is its maximum minus its current air. Without breath,
    /// air falls by `consume`, and at -20 it is set to 0
    /// and the entity takes 2 drowning damage. Negative amounts count as 0.
    /// Asked on servers for players and mobs. A water animal, axolotl or
    /// nautilus comes in with its own air logic's answer: where it breathes
    /// (water, or rain for an axolotl) `can_breathe` and a `refill` to its full
    /// air; elsewhere no breath and `consume` 1. Running out deals its own
    /// damage (drowning, or drying out). It is given the loop `context` of the server that ticks the entity (its registries, settings and mod list), because it fires inside the tick's own air update.
    #[hook(
        side = "server",
        contract = "listen",
        context = "loop",
        neoforge = "LivingBreatheEvent",
        cancellable = false,
        version = 3,
        event_per_listener = true,
        target = "sunrise_client_lib::breathing::native_living_breathe"
    )]
    fn living_breathe();
}
