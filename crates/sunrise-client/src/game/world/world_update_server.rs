#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::world_update::order_uploads;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::world_update::section_distance_sq;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::world_update::*;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn update_world_server(&mut self, dim: Dim) {
        self.streaming.extra.extend(
            self.server
                .server_trips
                .iter()
                .flat_map(|trip| trip.spawn_destination_areas(dim)),
        );
        let waiting = std::mem::take(&mut self.server.spawn_wait);
        self.streaming
            .extra
            .extend(waiting.into_iter().map(|chunk| (chunk, 0)));
    }
}
