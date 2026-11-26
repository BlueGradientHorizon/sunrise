use crate::game::spill_common::Spill;
use mod_api::{LevelRef, PlayerRef};
use sunrise_world::{fluid::FluidKind, level::FluidWrite};

/// A bucket's sound and fluid game event waiting for the outermost commit.
pub struct FluidEffects {
    pub(in crate::game) level: LevelRef,
    pub(in crate::game) pos: [i32; 3],
    /// The fluid's world form, whose bucket sounds play; none for a tank-only one.
    pub(in crate::game) fluid: Option<FluidKind>,
    /// A fill (`fluid_pickup`), else an empty (`fluid_place`).
    pub(in crate::game) pickup: bool,
    pub(in crate::game) player: Option<PlayerRef>,
}

/// A world pickup or placement decided at the call, its block already set in the transaction:
/// what the outermost commit still does.
pub struct WrittenFluid {
    pub(in crate::game) level: LevelRef,
    pub(in crate::game) write: FluidWrite,
    /// The broken block's drops, made from the block as it was at the call.
    pub(in crate::game) drops: Spill,
    /// A pickup (the bucket's fill sound and `fluid_pickup`), else a placement.
    pub(in crate::game) pickup: bool,
    pub(in crate::game) player: Option<PlayerRef>,
}
