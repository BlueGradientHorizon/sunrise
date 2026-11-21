//! The client thread's ordered ingress, shared by its packet processor and executor.
use super::{Event, ingress};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Application-lifetime client ingress. A connection borrows a producer, not the drain.
pub struct ClientInbox {
    sender: ingress::Producer<QueuedEvent>,
    receiver: Option<ingress::Consumer<QueuedEvent>>,
}

/// A connection's producer. The drain ignores its packets once that connection is gone.
#[derive(Clone)]
pub struct EventSender {
    serial: u64,
    closed: Arc<AtomicBool>,
    sender: ingress::Producer<QueuedEvent>,
}

/// A receiving turn item. Tasks belong to the application, not a connection.
pub struct QueuedEvent {
    pub serial: Option<u64>,
    pub event: Event,
}

impl Default for ClientInbox {
    fn default() -> Self {
        let (sender, receiver) = ingress::channel(96 << 20, |queued: &QueuedEvent| {
            queued.event.ingress_charge()
        })
        .expect("positive incoming byte budget");
        Self {
            sender,
            receiver: Some(receiver),
        }
    }
}

impl ClientInbox {
    pub(super) fn connection(&self, serial: u64) -> EventSender {
        EventSender {
            serial,
            closed: Arc::new(AtomicBool::new(false)),
            sender: self.sender.clone(),
        }
    }

    /// Take the events already published for this turn. Receipts keep their original credit.
    #[sunrise_mod_api_macros::replaceable]
    pub fn poll(&self) -> Vec<ingress::OwnedReceipt<QueuedEvent>> {
        self.receiver
            .as_ref()
            .map_or_else(Vec::new, ingress::Consumer::take_turn)
    }

    pub fn close(&mut self) {
        self.receiver = None;
    }

    pub fn executor(&self, owner: &'static str) -> Option<mod_api::ClientExecutor> {
        self.receiver.as_ref()?;
        let sender = self.sender.clone();
        Some(mod_api::ClientExecutor::new(move |run| {
            let _ = sender.send_local(QueuedEvent {
                serial: None,
                event: Event::ClientTask(ClientTask {
                    owner,
                    run: Some(run),
                }),
            });
        }))
    }

    #[cfg(test)]
    pub(super) fn into_receiver(self) -> ingress::Consumer<QueuedEvent> {
        self.receiver.unwrap()
    }

    #[cfg(test)]
    pub(super) fn receiver_for_test(&self) -> &ingress::Consumer<QueuedEvent> {
        self.receiver.as_ref().unwrap()
    }
}

type Task = Box<dyn FnOnce(&mut dyn mod_api::ClientHost) + Send>;

/// An executor callback, with the mod that obtained the handle.
pub struct ClientTask {
    pub owner: &'static str,
    run: Option<Task>,
}

impl ClientTask {
    pub fn run(mut self, host: &mut dyn mod_api::ClientHost) {
        if let Some(run) = self.run.take() {
            run(host);
        }
    }
}

impl Drop for ClientTask {
    fn drop(&mut self) {
        // A discarded closure can still have a mod-owned destructor. In particular a
        // submission racing shutdown must not unwind on its producer's thread.
        if let Some(run) = self.run.take()
            && let Err(error) = crate::crash::contain(|| drop(run))
        {
            log::error!(
                "Mod {} panicked dropping client executor work: {error}",
                self.owner
            );
        }
    }
}

impl EventSender {
    pub(super) fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub(crate) fn prepare(
        &self,
        event: Event,
    ) -> Result<ingress::Prepared<QueuedEvent>, ingress::Refused<QueuedEvent>> {
        let value = QueuedEvent {
            serial: Some(self.serial),
            event,
        };
        if self.closed.load(Ordering::Acquire) {
            return Err(ingress::Refused {
                reason: ingress::Refusal::Closed,
                value,
            });
        }
        self.sender.prepare(value)
    }

    pub(super) fn send(&self, event: Event) -> Result<(), ingress::Refused<QueuedEvent>> {
        self.prepare(event)?.publish()
    }
}
