#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::world_update::*;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn update_world_save_server(
        &mut self,
        modified: &[std::sync::Arc<sunrise_client::chunk::Chunk>],
    ) -> Result<usize> {
        sunrise_client::spans::time("save_chunks", || {
            self.save
                .0
                .save_later(modified, self.pack.assets.registry, world_biomes!(self))
        })
    }
    #[inline(always)]
    pub(in crate::game) fn update_world_server(&mut self, dim: Dim) {
        // The configuration connection is not a guest and owns no player area.
        // `extra` finishes chunks through radius + 1, even with no players.
        self.streaming.extra.extend(
            self.tickets
                .0
                .spawn_areas()
                .map(|(center, radius)| (center, radius - 1)),
        );
        if let Some(host) = &self.lan {
            self.streaming.extra.extend(
                host.hub
                    .lan
                    .configuration_tasks
                    .loading_spawns(dim.id())
                    .filter(|&center| !self.tickets.0.holds_player_spawn(center))
                    .map(|center| {
                        (
                            center,
                            sunrise_client::ticket_levels::PLAYER_SPAWN_RADIUS - 1,
                        )
                    }),
            );
        }
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
