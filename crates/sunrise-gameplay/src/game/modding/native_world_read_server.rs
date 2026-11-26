use sunrise_gameplay_common::game::native_world_read_common::engine_fluid_kind;
use sunrise_gameplay_common::game::{Dim, Game, GameLevel, native_level_signals::ReadLevel};
use mod_api::{FluidKey, FluidState, LevelRef, LifecycleContext};
use sunrise_core::position::BlockPos;
use sunrise_level::world::World;
use sunrise_world::{chunk::BlockState, level::fluid_tick_delay, registry::BlockRegistry};

/// Why a server block cannot be read.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub enum NativeBlockReadError {
    /// The context is stale or the call is outside its server tick hook.
    InactiveContext,
    /// The dimension is unknown or is not currently open on the server.
    UnavailableDimension,
    OutOfBounds,
    Unloaded,
    InvalidState,
}

impl crate::game::ServerGame<'_> {
    /// The fluid state at `pos` of the level `level` names (`BlockGetter.getFluidState`). None
    /// for a stale handle, a chunk that isn't loaded or a `y` outside the world: a read never
    /// loads a chunk.
    pub(in crate::game) fn level_fluid_state(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<FluidState> {
        let state = self.common.level_block_state(level, pos)?;
        self.common.state_fluid_state(state)
    }

    /// `Fluid.getTickDelay(level)` of `fluid` at `pos` in the level in place: vanilla's by its
    /// family and the dimension (`World::fast_lava`), a mod fluid's as registered and as its
    /// `fluid_rules` parts change it there (the one rule path the flow itself reads). None for
    /// another level, a stale handle, a client, an unloaded or out-of-height position and a
    /// fluid with no world form.
    pub(in crate::game) fn level_fluid_tick_delay(
        &self,
        level: LevelRef,
        fluid: FluidKey,
        pos: [i32; 3],
    ) -> Option<i32> {
        let [x, y, z] = pos;
        if !self.common.level_in_place(level) || !self.common.world.is_loaded_at(x, y, z) {
            return None;
        }
        let kind = engine_fluid_kind(fluid)?;
        let view = ReadLevel::new(self, level);
        Some(fluid_tick_delay(kind, self.common.world.fast_lava, || {
            let fluid = kind.mod_fluid()?;
            Some(GameLevel::answered_fluid_rules(
                self.common.dim.level(),
                BlockPos::new(x, y, z),
                fluid,
                fluid.get().rules,
                |call| mod_api::dispatch_fluid_rules(&view, call),
            ))
        }))
    }

    /// Read a loaded block in a server tick hook, without loading a chunk.
    /// The returned name and properties are owned and remain valid after the hook.
    #[allow(dead_code)]
    pub(in crate::game::server_side) fn native_server_block(
        &self,
        context: &LifecycleContext,
        dimension: &str,
        pos: [i32; 3],
    ) -> Result<BlockState, NativeBlockReadError> {
        if !self.common.native_lifecycle.permits_read(context) {
            return Err(NativeBlockReadError::InactiveContext);
        }
        if !Dim::exists(dimension) {
            return Err(NativeBlockReadError::UnavailableDimension);
        }
        let dim = Dim::from_id(dimension);
        let world = if dim == self.common.dim {
            Some(&self.common.world)
        } else {
            self.server.levels.0.parked_world(dim)
        }
        .ok_or(NativeBlockReadError::UnavailableDimension)?;
        read_block(world, self.common.pack.assets.registry, pos)
    }
}

/// Resolve an existing world state, preserving unloaded and invalid states.
pub fn read_block(
    world: &World,
    registry: &BlockRegistry,
    [x, y, z]: [i32; 3],
) -> Result<BlockState, NativeBlockReadError> {
    if !(-30_000_000..30_000_000).contains(&x)
        || !(-30_000_000..30_000_000).contains(&z)
        || !world.shape.contains_y(y)
    {
        return Err(NativeBlockReadError::OutOfBounds);
    }
    if !world.loaded(x, z) {
        return Err(NativeBlockReadError::Unloaded);
    }
    registry
        .state(world.block(x, y, z))
        .cloned()
        .ok_or(NativeBlockReadError::InvalidState)
}
