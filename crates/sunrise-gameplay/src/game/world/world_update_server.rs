use super::*;

impl crate::game::ServerGame<'_> {
    #[inline(always)]
    pub(in crate::game::server_side) fn update_world_save_server(
        &mut self,
        modified: &[std::sync::Arc<sunrise_level::chunk::Chunk>],
    ) -> Result<usize> {
        sunrise_level::spans::time("save_chunks", || {
            self.server.save.0.save_later(
                modified,
                self.common.pack.assets.registry,
                world_biomes!(self),
            )
        })
    }
    pub(in crate::game) fn server_clear_lod(&mut self) {}
    pub fn server_update_world(&mut self) {
        self.ask_edited_light_emission();
        sunrise_level::spans::time("tick_pregen", || self.tick_pregen());
        if self.defer_chunk_disk_save() {
            return;
        }
        let areas = self.server.streaming.extra.len();
        let dim = self.common.dim;
        self.update_world_server(dim);
        self.server.streaming.hold_unknown_tickets();
        let plan = sunrise_level::spans::time("streaming_plan", || {
            let mut step = self
                .server
                .streaming
                .begin_plan_update(&mut self.common.world);
            loop {
                match step {
                    sunrise_level::chunk_streamer::ChunkPlanStep::Ready(plan) => break plan,
                    sunrise_level::chunk_streamer::ChunkPlanStep::Save { chunk, resume } => {
                        self.native_late_full_chunk_snapshot(&chunk);
                        self.server.streaming.save_finished(chunk);
                        step = self
                            .server
                            .streaming
                            .resume_plan_update(&mut self.common.world, resume);
                    }
                }
            }
        });
        let (commit, protos) = self
            .server
            .streaming
            .prepare_commit(&mut self.common.world, plan);
        self.native_proto_snapshots(&protos);
        self.server.streaming.submit_protos(protos);
        let unloaded = sunrise_level::spans::time("streaming", || {
            self.server
                .streaming
                .complete_commit(&mut self.common.world, commit)
        });
        self.server.streaming.extra.truncate(areas);
        let dirty: Vec<_> = unloaded
            .iter()
            .filter(|c| self.common.world.modified.remove(&(c.x, c.z)))
            .cloned()
            .collect();
        let modified = sunrise_level::spans::time("with_entities", || self.with_entities(&dirty));
        self.native_full_chunk_snapshots(&modified);
        if !modified.is_empty()
            && let Err(e) = self.update_world_save_server(&modified)
        {
            log::warn!("Sunrise: failed to save unloaded chunks: {e}");
        }
        sunrise_level::spans::time("spawn_fresh_chunks", || self.spawn_fresh_chunks());
        self.ask_loaded_light_emission();
        sunrise_level::spans::time("mods_chunks_changed", || {
            self.native_chunks_loaded();
            self.native_chunks_unloaded(&unloaded);
        });
        for chunk in &unloaded {
            self.common.world.modified.unloaded(&(chunk.x, chunk.z));
        }
        sunrise_level::spans::time("unload_entities", || self.unload_entities(&unloaded));
        if !unloaded.is_empty() {
            let gone: sunrise_level::fast_hash::FxHashSet<(i32, i32)> =
                unloaded.iter().map(|c| (c.x, c.z)).collect();
            sunrise_level::spans::time("unload_block_entities", || {
                for &column in &gone {
                    self.common.world.block_entities.unload_column(column);
                }
            });
            self.common
                .world
                .block_names
                .retain(|p, _| !gone.contains(&(p.0 >> 4, p.2 >> 4)));
        }
        sunrise_level::spans::time("sync_tick_chunks", || self.sync_tick_chunks());
    }
    #[inline(always)]
    pub(in crate::game::server_side) fn update_world_server(&mut self, dim: Dim) {
        // The configuration connection is not a guest and owns no player area.
        // `extra` finishes chunks through radius + 1, even with no players.
        self.server.streaming.extra.extend(
            self.server
                .tickets
                .0
                .spawn_areas()
                .map(|(center, radius)| (center, radius - 1)),
        );
        if let Some(host) = &self.server.lan {
            self.server.streaming.extra.extend(
                host.hub
                    .lan
                    .configuration_tasks
                    .loading_spawns(dim.id())
                    .filter(|&center| !self.server.tickets.0.holds_player_spawn(center))
                    .map(|center| (center, sunrise_sim::ticket_levels::PLAYER_SPAWN_RADIUS - 1)),
            );
        }
        self.server.streaming.extra.extend(
            self.server
                .server_trips
                .iter()
                .flat_map(|trip| trip.spawn_destination_areas(dim)),
        );
        let waiting = std::mem::take(&mut self.server.spawn_wait);
        self.server
            .streaming
            .extra
            .extend(waiting.into_iter().map(|chunk| (chunk, 0)));
    }
}

use sunrise_gameplay_common::game::light_emission::LightEmissionGameExt1 as _;

use sunrise_gameplay_common::game::native_hooks::SharedGameExt1 as _;
