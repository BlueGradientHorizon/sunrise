#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::light_emission::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::fast_hash::FxHashMap;

impl Game<'_> {
    /// Re-query dynamic-light cells in sections touched by a server light update.
    pub(in crate::game) fn ask_updated_light_emission(
        &mut self,
        key: (i32, i32),
        sections: &[usize],
    ) {
        if sections.is_empty() || !self.pack.context.light.any_dynamic() {
            return;
        }
        let Some(chunk) = self.world.chunks.get(&key) else {
            return;
        };
        let cells = self
            .pack
            .context
            .light
            .dynamic_cells(chunk, |s| sections.contains(&s));
        let dimension = self.current_light_emission_dimension();
        for (cell, [x, y, z], state) in cells {
            let emission = self.ask_light_emission(dimension, x, y, z, state);
            self.store_light_emission(key, cell, emission);
        }
    }
}
