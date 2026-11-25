//! Role-neutral dynamic light emission queries for the level in place.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use sunrise_client::fast_hash::FxHashMap;

impl Game<'_> {
    /// Query dynamic light emission for chunks newly loaded into this level.
    pub(in crate::game) fn ask_loaded_light_emission(&mut self) {
        if !self.pack.context.light.any_dynamic() {
            return;
        }
        for i in 0..self.world.loaded_chunks.len() {
            let (x, z, _) = self.world.loaded_chunks[i];
            self.ask_chunk_light_emission((x, z));
        }
    }

    /// Query every dynamic-light cell of resident chunk `key` and apply the answers.
    pub(in crate::game) fn ask_chunk_light_emission(&mut self, key: (i32, i32)) {
        let dimension = self.current_light_emission_dimension();
        let Some(chunk) = self.world.chunks.get(&key) else {
            return;
        };
        let pending = chunk.dynamic_pending;
        let cells = self.pack.context.light.dynamic_cells(chunk, |_| true);
        if cells.is_empty() && !pending {
            return;
        }
        let mut answers = FxHashMap::default();
        for (cell, [x, y, z], state) in cells {
            if let Some(emission) = self.ask_light_emission(dimension, x, y, z, state) {
                answers.insert(cell, emission);
            }
        }
        Self::apply_chunk_light_emission(&mut self.world, key, pending, answers);
    }

    /// Store dynamic-light answers on a chunk and mark it for relight when they changed.
    pub(in crate::game) fn apply_chunk_light_emission(
        world: &mut sunrise_client::world::World,
        key: (i32, i32),
        pending: bool,
        answers: FxHashMap<u32, u8>,
    ) {
        if let Some(chunk) = world.chunks.get_mut(&key)
            && (pending || chunk.dynamic_emission != answers)
        {
            let relight = chunk.light_loaded && chunk.dynamic_emission != answers;
            let chunk = Arc::make_mut(chunk);
            chunk.dynamic_emission = answers;
            chunk.dynamic_pending = false;
            if relight {
                world.dirty_light.insert(key);
            }
        }
    }

    /// Query dynamic-light cells affected by this update's light edits before relighting.
    pub(in crate::game) fn ask_edited_light_emission(&mut self) {
        self.world.light_checks.clear();
        if self.world.light_edits.is_empty() || !self.pack.context.light.any_dynamic() {
            return;
        }
        let dimension = self.current_light_emission_dimension();
        for i in 0..self.world.light_edits.len() {
            let ((x, y, z), _, _) = self.world.light_edits[i];
            let state = self.world.block(x, y, z).0;
            if !self.pack.context.light.is_dynamic(state) {
                continue;
            }
            let emission = self.ask_light_emission(dimension, x, y, z, state);
            let key = (x >> 4, z >> 4);
            if let Some(cell) = self
                .world
                .chunks
                .get(&key)
                .and_then(|c| c.dynamic_cell(x, y, z))
            {
                self.store_light_emission(key, cell, emission);
            }
        }
    }

    /// Store one cell's dynamic-light emission in its chunk when the answer changed.
    pub(in crate::game) fn store_light_emission(
        &mut self,
        key: (i32, i32),
        cell: u32,
        emission: Option<u8>,
    ) {
        let Some(chunk) = self.world.chunks.get_mut(&key) else {
            return;
        };
        if chunk.dynamic_emission.get(&cell).copied() == emission {
            return;
        }
        let answers = &mut Arc::make_mut(chunk).dynamic_emission;
        match emission {
            Some(emission) => answers.insert(cell, emission),
            None => answers.remove(&cell),
        };
    }

    /// Dimension of the level in play, including a client's custom dimension.
    pub(in crate::game) fn current_light_emission_dimension(&self) -> LevelDimension {
        self.native_current_level()
            .map_or_else(|| self.dim.level(), Self::light_emission_dimension)
    }

    /// Query one cell's dynamic light emission, clamped to 15.
    pub(in crate::game) fn ask_light_emission(
        &self,
        dimension: LevelDimension,
        x: i32,
        y: i32,
        z: i32,
        state: u32,
    ) -> Option<u8> {
        self.query_dynamic_light_emission(BlockPos::new(x, y, z), BlockStateId(state), || dimension)
            .flatten()
            .map(|emission| emission.min(15))
    }
}
