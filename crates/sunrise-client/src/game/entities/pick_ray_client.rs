#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, Hit};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::pick_ray::PickAccumulator;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::pick_ray::PickCandidate;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::pick_ray::pick_ray_segment;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::pick_ray::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::{Aabb, Vec3};

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
