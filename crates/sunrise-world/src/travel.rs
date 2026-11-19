//! Which `LivingEntity.travel` branch a living body is about to run — the
//! game-side twin of the mod API's `TravelMode` (the `living_travel` hook of
//! #2308/#2414), kept separate so the physics below the game stays free of
//! mod-API types, as `entity_move::MoverType` is.

/// Which `LivingEntity.travel` branch is about to run (`living_travel` of
/// #2308/#2414): the fluid branch in water or in lava, the glide branch, the
/// flying branch, or ordinary air travel. The hook resolves it before the
/// branch selection runs and passes it to the listener as data; a listener
/// that replaces the travel's input or velocity does not change which branch
/// runs — the branch is the game's own, as the design rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TravelMode {
    /// `LivingEntity.travelInAir`: the ordinary ground-and-air branch.
    Air,
    /// `LivingEntity.travelInFluid` with water: swimming.
    Water,
    /// `LivingEntity.travelInFluid` with lava.
    Lava,
    /// `LivingEntity.travelFallFlying`: gliding.
    Glide,
    /// The flying branch (`Player` abilities flying, a flying mob kind).
    Flying,
}
