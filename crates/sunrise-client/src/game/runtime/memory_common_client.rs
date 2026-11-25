#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::Allocator;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::Cached;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::ChunkSend;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::Columns;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::Entities;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::FarTiles;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::Generation;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::Gpu;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::MB;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::MEMORY_EVERY_S;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::MemoryCancellation;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::MemoryInputs;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::MemoryReport;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::MemoryRows;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::PendingMemory;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::Players;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::Process;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::Queue;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::Renderer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::ServerMemoryRequest;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::ServerSide;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::SharedSections;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::TOP;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::TOP_PLAYERS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::census;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::flatten;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::flatten_report;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::mb;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::mb_signed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::shared;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::unattributed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use serde::Serialize;
#[allow(unused_imports)]
pub(in crate::game::client_side) use serde_json::Value;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::BTreeMap;
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
pub(in crate::game::client_side) use sunrise_client::server_perf::MemoryCost;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client_render::gfx::RendererMemorySnapshot;

impl MemoryCancellation {
    pub(in crate::game) fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(wake) = self.wake_worker.take() {
            let _ = wake.send(None);
        }
    }
}

impl PendingMemory {
    pub(in crate::game) fn cancel(&mut self) {
        self.cancel.cancel();
    }
    pub(in crate::game) fn poll(&self) -> Poll<Option<MemoryRows>> {
        // Keep the slot reserved while either the worker or queued request still holds the flag.
        if !self.worker.is_finished() || Arc::strong_count(&self.cancel.cancelled) != 1 {
            return Poll::Pending;
        }
        if self.cancel.cancelled.load(Ordering::Acquire) {
            return Poll::Ready(None);
        }
        Poll::Ready(self.rows.try_recv().ok())
    }
    pub(in crate::game) fn spawn(
        cost: MemoryCost,
        from_server: bool,
        report: impl FnOnce(Option<ServerSide>, &AtomicBool) -> Option<MemoryRows> + Send + 'static,
    ) -> std::io::Result<(Self, Option<ServerMemoryRequest>)> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (reply, snapshot) = channel();
        let request = from_server.then(|| ServerMemoryRequest {
            reply: Some(reply.clone()),
            cancelled: cancelled.clone(),
            cost: cost.clone(),
        });
        let (publish, rows) = sync_channel(1);
        let worker = Builder::new().name("perf-memory".into()).spawn({
            let cancelled = cancelled.clone();
            move || {
                let server = if from_server {
                    let Ok(Some(snapshot)) = snapshot.recv() else {
                        return;
                    };
                    Some(snapshot)
                } else {
                    None
                };
                if cancelled.load(Ordering::Acquire) {
                    return;
                }
                sunrise_client::server_perf::profiler_thread();
                let completed = cost.measure(|| report(server, &cancelled));
                if let Some(rows) = completed.filter(|_| !cancelled.load(Ordering::Acquire)) {
                    let _ = publish.send(rows);
                }
            }
        })?;
        let cancel = MemoryCancellation {
            cancelled,
            wake_worker: from_server.then_some(reply),
        };
        Ok((
            Self {
                cancel,
                worker,
                rows,
            },
            request,
        ))
    }
}
