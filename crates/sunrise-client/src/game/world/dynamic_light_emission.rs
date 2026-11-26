//! `light_emission`, the light a `dynamic_light` block emits by where it is
//! (`IBlockExtension.getLightEmission`): asked of its parts on the thread that owns the
//! level as a chunk holding one comes in, as an edit or `level_check_light` reaches one, and
//! on a client as the server's light update reaches its section, the answers kept in the
//! chunk (`Chunk::dynamic_emission`) for the light engine, which never asks on its own.
use super::*;
use sunrise_level::fast_hash::FxHashMap;

impl Game<'_, crate::game::server_side::ServerState> {
    /// Query dynamic light emission in the active (`None`) or parked (`Some`) level.
    pub(in crate::game::server_side) fn ask_level_chunk_light_emission(
        &mut self,
        parked: Option<&str>,
        dimension: LevelDimension,
        key: (i32, i32),
    ) {
        let world = match parked {
            None => &self.world,
            Some(id) => match self.side.levels.0.parked_world_of(id) {
                Some(world) => world,
                None => return,
            },
        };
        let Some(chunk) = world.chunks.get(&key) else {
            return;
        };
        let pending = chunk.dynamic_pending;
        let cells = self.pack.chunk_context.light.dynamic_cells(chunk, |_| true);
        if cells.is_empty() && !pending {
            return;
        }
        let mut answers = FxHashMap::default();
        for (cell, [x, y, z], state) in cells {
            if let Some(emission) = self.ask_light_emission(dimension, x, y, z, state) {
                answers.insert(cell, emission);
            }
        }
        let world = match parked {
            None => &mut self.world,
            Some(id) => match self.side.levels.0.parked_world_mut_of(id) {
                Some(world) => world,
                None => return,
            },
        };
        Self::apply_chunk_light_emission(world, key, pending, answers);
    }
}
