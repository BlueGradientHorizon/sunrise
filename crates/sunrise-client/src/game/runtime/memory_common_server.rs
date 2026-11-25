#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::Allocator;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::Cached;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::ChunkSend;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::Columns;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::Entities;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::FarTiles;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::Generation;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::Gpu;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::MB;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::MEMORY_EVERY_S;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::MemoryCancellation;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::MemoryInputs;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::MemoryReport;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::MemoryRows;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::PendingMemory;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::Players;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::Process;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::Queue;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::Renderer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::ServerMemoryRequest;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::ServerSide;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::SharedSections;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::TOP;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::TOP_PLAYERS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::census;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::flatten;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::flatten_report;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::mb;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::mb_signed;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::shared;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::unattributed;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::memory_common::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde::Serialize;
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde_json::Value;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::BTreeMap;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, Sender, channel, sync_channel},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::task::Poll;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::thread::{Builder, JoinHandle};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::chunk::{Chunk, ColumnCensus, Section};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::fast_hash::FxHashSet;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::server_perf::MemoryCost;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_render::gfx::RendererMemorySnapshot;

impl Drop for ServerMemoryRequest {
    fn drop(&mut self) {
        // The server may discard the request on shutdown without handling it.
        if let Some(reply) = self.reply.take() {
            let _ = reply.send(None);
        }
    }
}

impl Generation {
    pub(in crate::game) fn of(streaming: &sunrise_client::streaming::Streaming) -> Self {
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
