use super::Game;
use super::owner_link::{ControlRequest, OwnerRequest};
use crate::game::memory_common::*;
use serde_json::Value;
use std::sync::atomic::Ordering;
use sunrise_client::server_perf::MemoryCost;

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn client_memory_inputs(&self) -> MemoryInputs {
        let far = self.side.lod.0.as_ref().map(|lod| {
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
            render_distance: self.side.streaming.radius,
            client: self
                .options
                .dedicated
                .is_none()
                .then(|| self.world.chunks.values().cloned().collect()),
            renderer: self.side.renderer.0.as_ref().map(|r| r.memory_snapshot()),
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
    pub(in crate::game) fn start(
        game: &Game<'_, crate::game::client_side::ClientState>,
        cost: MemoryCost,
    ) -> Option<Self> {
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
