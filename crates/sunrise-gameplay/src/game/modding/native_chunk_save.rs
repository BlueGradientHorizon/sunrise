//! Server capture seams. Disk callbacks run only in the persistence owner.
use super::*;
use sunrise_level::chunk::Chunk;
use sunrise_level::chunk_streamer::{ProtoSave, ProtoSnapshot};

impl crate::game::ServerGame<'_> {
    /// Bind the existing world's identity and immutable resources, before workers start.
    pub(in crate::game::server_side) fn bind_chunk_disk_worker(
        &self,
        save: &sunrise_sim::save::SaveDir,
        dim: super::Dim,
    ) {
        if self.common.options.execution_role.is_client()
            || (!mod_api::__private::listened(mod_api::HookId::ChunkDataLoad)
                && !mod_api::__private::listened(mod_api::HookId::ChunkDataSave))
        {
            return;
        }
        if let (Some(world), Some(resources)) = (
            self.common.native_lifecycle.epoch(),
            sunrise_world::native_worldgen_hooks::context(),
        ) {
            save.set_chunk_disk_context(
                world,
                mod_api::Identifier::parse(dim.id()).expect("registered dimension"),
                resources,
            );
        }
    }

    /// Observe the exact full-chunk save copies on the server thread.
    pub(in crate::game::server_side) fn native_full_chunk_snapshots(
        &mut self,
        chunks: &[Arc<Chunk>],
    ) {
        if !self.common.is_logical_server() || mod_api::__private::mods().is_empty() {
            return;
        }
        for chunk in chunks {
            self.native_full_chunk_snapshot(chunk);
        }
    }

    pub(in crate::game) fn native_late_full_chunk_snapshot(&mut self, chunk: &Chunk) {
        if self.common.is_logical_server() && !mod_api::__private::mods().is_empty() {
            self.native_full_chunk_snapshot(chunk);
        }
    }

    fn native_full_chunk_snapshot(&mut self, _chunk: &Chunk) {
        self.bind_chunk_disk_worker(&self.server.save.0, self.common.dim);
    }

    /// The proto stays checked out until its immutable save record is submitted.
    pub(in crate::game) fn native_proto_snapshots(&mut self, protos: &[ProtoSave]) {
        if !self.common.is_logical_server() || mod_api::__private::mods().is_empty() {
            return;
        }
        let access = self.server.streaming.proto_access();
        let dimension = self.common.dim.id();
        for save in protos {
            let Some(status) = access.status(&save.proto) else {
                continue;
            };
            self.native_proto_snapshot(ProtoSnapshot {
                dimension,
                pos: [save.pos.0, save.pos.1],
                proto: &save.proto,
                access: access.as_ref(),
                status,
            });
        }
    }

    fn native_proto_snapshot(&mut self, _snapshot: ProtoSnapshot<'_>) {
        self.bind_chunk_disk_worker(&self.server.save.0, self.common.dim);
    }
}

impl crate::game::ServerGame<'_> {
    /// With a `chunk_data_save` listener, saves wait while an action savepoint
    /// is open, so tentative state is never captured: unloads, and the plan
    /// that causes them, are deferred until it closes (the design's "defer
    /// unload when the gate refuses"). Never defers without a listener.
    pub(in crate::game) fn defer_chunk_disk_save(&mut self) -> bool {
        if self.common.options.execution_role.is_client()
            || !mod_api::__private::listened(mod_api::HookId::ChunkDataSave)
        {
            return false;
        }
        Self::admit_entity_save().is_err()
    }

    /// `ChunkMap.saveChunksEagerly`: snapshot only while the tick and writer
    /// have room. NBT encoding, compression and durable writes stay on workers.
    pub(in crate::game::server_side) fn save_chunks_eagerly(&mut self) {
        self.save_chunks_eagerly_at(Instant::now());
    }

    fn save_chunks_eagerly_at(&mut self, now: Instant) {
        // No world open (`world_root` is set by `finish_open_world`): the save
        // is `Game::new`'s placeholder under the working directory.
        if self.common.world.carrier_fences.is_broken()
            || self.server.world_root.as_os_str().is_empty()
            || self.server.hold_server
            || self.common.clock.sprinting()
            || self
                .server
                .server_lists
                .0
                .as_ref()
                .is_some_and(|lists| !lists.lock().unwrap().autosave)
        {
            return;
        }
        if self.defer_chunk_disk_save() {
            return;
        }
        let deadline = self
            .server
            .server_tick_deadline
            .unwrap_or_else(|| Instant::now() + self.common.clock.tick_duration());
        let mut modified = std::mem::take(&mut self.common.world.modified);
        modified.save_eagerly(
            now,
            || Instant::now() < deadline && self.server.save.0.active_chunk_writes() < 128,
            // Only a lit column is a LevelChunk. An unlit one stays queued.
            |key| self.common.world.chunks.get(&key).map(|chunk| chunk.lit),
            |key, _| {
                let chunks = self.with_entities([&self.common.world.chunks[&key]]);
                match self.server.save.0.save_later(
                    &chunks,
                    self.common.pack.assets.registry,
                    world_biomes!(self),
                ) {
                    Ok(_) => true,
                    Err(e) => {
                        log::warn!("Sunrise: failed to save changed chunks: {e}");
                        false
                    }
                }
            },
        );
        self.common.world.modified = modified;
    }
}
