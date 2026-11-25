use super::*;

impl Game<'_, crate::game::client_side::ClientState> {
    /// The columns this update's block edits touch, summarized again by far terrain when
    /// it is on.
    pub(in crate::game::client_side) fn observe_light_edits(&mut self) {
        if self.side.lod.0.is_some() {
            for &((x, _, z), _, _) in &self.world.light_edits {
                self.side.lod_observe.insert((x >> 4, z >> 4));
            }
        }
    }
}
