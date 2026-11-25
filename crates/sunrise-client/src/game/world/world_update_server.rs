#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::world_update::*;

impl Game<'_, crate::game::server_side::ServerState> {
    #[inline(always)]
    pub(in crate::game::server_side) fn update_world_save_server(
        &mut self,
        modified: &[std::sync::Arc<sunrise_client::chunk::Chunk>],
    ) -> Result<usize> {
        sunrise_client::spans::time("save_chunks", || {
            self.side
                .save
                .0
                .save_later(modified, self.pack.assets.registry, world_biomes!(self))
        })
    }
    pub(in crate::game) fn server_clear_lod(&mut self) {}
    pub(in crate::game) fn server_update_world(&mut self) {
        self.ask_edited_light_emission();
        sunrise_client::spans::time("tick_pregen", || self.tick_pregen());
        if self.defer_chunk_disk_save() {
            return;
        }
        let areas = self.streaming.extra.len();
        let dim = self.dim;
        self.update_world_server(dim);
        self.streaming.hold_unknown_tickets();
        let plan = sunrise_client::spans::time("streaming_plan", || {
            let mut step = self.streaming.begin_plan_update(&mut self.world);
            loop {
                match step {
                    sunrise_client::streaming::PlanStep::Ready(plan) => break plan,
                    sunrise_client::streaming::PlanStep::Save { chunk, resume } => {
                        self.native_late_full_chunk_snapshot(&chunk);
                        self.streaming.save_finished(chunk);
                        step = self.streaming.resume_plan_update(&mut self.world, resume);
                    }
                }
            }
        });
        let (commit, protos) = self.streaming.prepare_commit(&mut self.world, plan);
        self.native_proto_snapshots(&protos);
        self.streaming.submit_protos(protos);
        let (_uploads, unloaded) = sunrise_client::spans::time("streaming", || {
            self.streaming.complete_commit(&mut self.world, commit)
        });
        self.streaming.extra.truncate(areas);
        let dirty: Vec<_> = unloaded
            .iter()
            .filter(|c| self.world.modified.remove(&(c.x, c.z)))
            .cloned()
            .collect();
        let modified = sunrise_client::spans::time("with_entities", || self.with_entities(&dirty));
        self.native_full_chunk_snapshots(&modified);
        if !modified.is_empty()
            && let Err(e) = self.update_world_save_server(&modified)
        {
            log::warn!("Sunrise: failed to save unloaded chunks: {e}");
        }
        sunrise_client::spans::time("spawn_fresh_chunks", || self.spawn_fresh_chunks());
        self.ask_loaded_light_emission();
        sunrise_client::spans::time("mods_chunks_changed", || {
            self.native_chunks_loaded();
            self.native_chunks_unloaded(&unloaded);
        });
        for chunk in &unloaded {
            self.world.modified.unloaded(&(chunk.x, chunk.z));
        }
        sunrise_client::spans::time("unload_entities", || self.unload_entities(&unloaded));
        if !unloaded.is_empty() {
            let gone: sunrise_client::fast_hash::FxHashSet<(i32, i32)> =
                unloaded.iter().map(|c| (c.x, c.z)).collect();
            sunrise_client::spans::time("unload_block_entities", || {
                for &column in &gone {
                    self.world.block_entities.unload_column(column);
                }
            });
            self.world
                .block_names
                .retain(|p, _| !gone.contains(&(p.0 >> 4, p.2 >> 4)));
        }
        sunrise_client::spans::time("sync_tick_chunks", || self.sync_tick_chunks());
    }
    #[inline(always)]
    pub(in crate::game::server_side) fn update_world_server(&mut self, dim: Dim) {
        // The configuration connection is not a guest and owns no player area.
        // `extra` finishes chunks through radius + 1, even with no players.
        self.streaming.extra.extend(
            self.side
                .tickets
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
                    .filter(|&center| !self.side.tickets.0.holds_player_spawn(center))
                    .map(|center| {
                        (
                            center,
                            sunrise_client::ticket_levels::PLAYER_SPAWN_RADIUS - 1,
                        )
                    }),
            );
        }
        self.streaming.extra.extend(
            self.side
                .server_trips
                .iter()
                .flat_map(|trip| trip.spawn_destination_areas(dim)),
        );
        let waiting = std::mem::take(&mut self.side.spawn_wait);
        self.streaming
            .extra
            .extend(waiting.into_iter().map(|chunk| (chunk, 0)));
    }
}
