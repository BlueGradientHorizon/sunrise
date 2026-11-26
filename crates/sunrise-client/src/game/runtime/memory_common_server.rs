use crate::game::memory_common::Generation;
use crate::game::memory_common::ServerMemoryRequest;
use crate::game::memory_common::ServerSide;
use crate::game::memory_common::mb;
use std::sync::atomic::Ordering;

impl Drop for ServerMemoryRequest {
    fn drop(&mut self) {
        // The server may discard the request on shutdown without handling it.
        if let Some(reply) = self.reply.take() {
            let _ = reply.send(None);
        }
    }
}

impl Generation {
    pub(in crate::game) fn of(streaming: &sunrise_level::chunk_streamer::ChunkStreamer) -> Self {
        let c = streaming.proto_census();
        let [blocks, lists, heightmaps, other, biomes, records] = c.bytes;
        let protos: usize = c.bytes.iter().sum();
        let cached: usize = c.caches.iter().filter_map(|(_, _, bytes)| *bytes).sum();
        Self {
            protos: c.protos,
            protos_mb: mb(protos),
            proto_blocks_mb: mb(blocks),
            proto_section_lists_mb: mb(lists),
            proto_heightmaps_mb: mb(heightmaps),
            proto_other_terrain_mb: mb(other),
            proto_biomes_mb: mb(biomes),
            proto_records_mb: mb(records),
            proto_sections_by_bits: c.sections_by_bits,
            light_views: c.light_views,
            light_views_mb: mb(c.light_view_bytes),
            caches: c
                .caches
                .iter()
                .map(|(name, n, bytes)| (*name, *n, bytes.map(mb)))
                .collect(),
            stages: streaming.stages().to_vec(),
            bytes: protos + c.light_view_bytes + cached,
        }
    }
}

impl ServerMemoryRequest {
    pub(in crate::game) fn collect(mut self, collect: impl FnOnce() -> ServerSide) {
        if self.cancelled.load(Ordering::Acquire) {
            return;
        }
        let snapshot = self.cost.measure(collect);
        if !self.cancelled.load(Ordering::Acquire) {
            let _ = self.reply.take().unwrap().send(Some(snapshot));
        }
    }
}
