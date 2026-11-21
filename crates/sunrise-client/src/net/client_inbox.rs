//! The client thread's ordered ingress, shared by its packet processor and executor.
use super::{Event, ingress};

/// Application-lifetime client ingress. A connection borrows a producer, not the drain.
pub struct ClientInbox {
    sender: ingress::Producer<QueuedEvent>,
    receiver: ingress::Consumer<QueuedEvent>,
}

/// A connection's producer. The drain ignores its packets once that connection is gone.
#[derive(Clone)]
pub struct EventSender {
    serial: u64,
    sender: ingress::Producer<QueuedEvent>,
}

/// A receiving turn item with its physical connection.
pub struct QueuedEvent {
    pub serial: u64,
    pub event: Event,
}

impl Default for ClientInbox {
    fn default() -> Self {
        let (sender, receiver) = ingress::channel(96 << 20, |queued: &QueuedEvent| {
            queued.event.ingress_charge()
        })
        .expect("positive incoming byte budget");
        Self { sender, receiver }
    }
}

impl ClientInbox {
    pub(super) fn connection(&self, serial: u64) -> EventSender {
        EventSender {
            serial,
            sender: self.sender.clone(),
        }
    }

    /// Take the events already published for this turn. Receipts keep their original credit.
    #[sunrise_mod_api_macros::replaceable]
    pub fn poll(&self) -> Vec<ingress::OwnedReceipt<QueuedEvent>> {
        self.receiver.take_turn()
    }

    #[cfg(test)]
    pub(super) fn into_receiver(self) -> ingress::Consumer<QueuedEvent> {
        self.receiver
    }

    #[cfg(test)]
    pub(super) fn receiver_for_test(&self) -> &ingress::Consumer<QueuedEvent> {
        &self.receiver
    }
}

impl EventSender {
    pub(crate) fn prepare(
        &self,
        event: Event,
    ) -> Result<ingress::Prepared<QueuedEvent>, ingress::Refused<QueuedEvent>> {
        self.sender.prepare(QueuedEvent {
            serial: self.serial,
            event,
        })
    }

    pub(super) fn send(&self, event: Event) -> Result<(), ingress::Refused<QueuedEvent>> {
        self.prepare(event)?.publish()
    }
}
