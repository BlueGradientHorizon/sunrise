#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::owner_link::{ControlRequest, OwnerRequest};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::memory_common::*;
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
    pub(in crate::game) fn client_memory_inputs(&self) -> MemoryInputs {
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

    pub(in crate::game) fn client_memory_report(&self) -> MemoryReport {
        let inputs = self.client_memory_inputs();
        let server = self
            .integrated
            .as_ref()
            .and_then(|i| i.ask(|reply| OwnerRequest::Control(ControlRequest::ServerSide(reply))));
        inputs
            .report(server, || true)
            .expect("a manual census cannot be canceled")
    }
}

impl PendingMemory {
    pub(in crate::game) fn start(game: &Game<'_>, cost: MemoryCost) -> Option<Self> {
        cost.measure(|| {
            let inputs = game.client_memory_inputs();
            let (job, request) = match Self::spawn(
                cost.clone(),
                game.integrated.is_some(),
                move |server, cancelled| {
                    let report = inputs.report(server, || !cancelled.load(Ordering::Acquire))?;
                    Some(flatten_report(
                        &serde_json::to_value(report).unwrap_or(Value::Null),
                    ))
                },
            ) {
                Ok(job) => job,
                Err(error) => {
                    log::warn!("Sunrise: could not start the memory census: {error}");
                    return None;
                }
            };
            if let Some((server, request)) = game.integrated.as_ref().zip(request) {
                server.tell(OwnerRequest::Control(ControlRequest::MemorySample(request)));
            }
            Some(job)
        })
    }
}
