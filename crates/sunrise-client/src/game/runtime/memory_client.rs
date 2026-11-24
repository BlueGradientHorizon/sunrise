#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::owner_link::{ControlRequest, OwnerRequest};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::Allocator;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::Cached;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::ChunkSend;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::Columns;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::Entities;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::FarTiles;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::Generation;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::Gpu;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::MB;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::MEMORY_EVERY_S;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::MemoryCancellation;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::MemoryInputs;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::MemoryReport;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::MemoryRows;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::PendingMemory;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::Players;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::Process;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::Queue;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::Renderer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::ServerMemoryRequest;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::ServerSide;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::SharedSections;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::TOP;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::TOP_PLAYERS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::biggest;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::census;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::chunk_send_census;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::flatten;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::flatten_report;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::mb;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::mb_signed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::player_census;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::shared;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::string_bytes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::unattributed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::vec_bytes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::memory::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use serde::Serialize;
#[allow(unused_imports)]
pub(in crate::game::client_side) use serde_json::Value;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::BTreeMap;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::mem::size_of;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, Sender, channel, sync_channel},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::task::Poll;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::thread::{Builder, JoinHandle};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::chunk::{Chunk, ColumnCensus, Section};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::fast_hash::FxHashSet;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::lan::hub::Hub;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::server_perf::MemoryCost;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client_render::gfx::RendererMemorySnapshot;

impl Game<'_> {
    pub(in crate::game) fn memory_inputs(&self) -> MemoryInputs {
        let far = self.lod.0.as_ref().map(|lod| {
            let ((active, cached), (kept, kept_bytes)) = (lod.stats(), lod.kept());
            let tiles = FarTiles {
                active,
                cached,
                kept,
                kept_mb: mb(kept_bytes),
            };
            (tiles, kept_bytes)
        });
        MemoryInputs {
            render_distance: self.streaming.radius,
            client: self
                .options
                .dedicated
                .is_none()
                .then(|| self.census_chunks()),
            renderer: self.renderer.0.as_ref().map(|r| r.memory_snapshot()),
            far,
        }
    }
}
