use crate::game::memory_common::MemoryCancellation;
use crate::game::memory_common::MemoryRows;
use crate::game::memory_common::PendingMemory;
use crate::game::memory_common::ServerMemoryRequest;
use crate::game::memory_common::ServerSide;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{channel, sync_channel},
};
use std::task::Poll;
use std::thread::Builder;
use sunrise_sim::server_perf::MemoryCost;

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
                sunrise_sim::server_perf::profiler_thread();
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
