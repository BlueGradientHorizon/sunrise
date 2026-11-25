use crate::game::pick_ray::PickAccumulator;
use crate::game::pick_ray::PickCandidate;

impl PickAccumulator {
    pub(in crate::game) fn nearest_sq(&self) -> f64 {
        self.nearest_sq
    }
    pub(in crate::game::client_side) fn recording_with(
        max_distance_sq: f64,
        mut candidates: Vec<PickCandidate>,
    ) -> Self {
        candidates.clear();
        Self {
            candidates: Some(candidates),
            ..Self::new(max_distance_sq)
        }
    }
    pub(in crate::game::client_side) fn take_candidates(&mut self) -> Vec<PickCandidate> {
        self.candidates.take().unwrap_or_default()
    }
}
