use crate::game::spill_common::Spill;
use mod_api::{LevelRef, PlayerRef};
use sunrise_world::{fluid::FluidKind, level::FluidWrite};

/// A bucket's sound and fluid game event waiting for the outermost commit.
pub struct FluidEffects {
    pub level: LevelRef,
    pub pos: [i32; 3],
    /// The fluid's world form, whose bucket sounds play; none for a tank-only one.
    pub fluid: Option<FluidKind>,
    /// A fill (`fluid_pickup`), else an empty (`fluid_place`).
    pub pickup: bool,
    pub player: Option<PlayerRef>,
}

/// A world pickup or placement decided at the call, its block already set in the transaction:
/// what the outermost commit still does.
pub struct WrittenFluid {
    pub level: LevelRef,
    pub write: FluidWrite,
    /// The broken block's drops, made from the block as it was at the call.
    pub drops: Spill,
    /// A pickup (the bucket's fill sound and `fluid_pickup`), else a placement.
    pub pickup: bool,
    pub player: Option<PlayerRef>,
}
