//! The client thread's ordered ingress, shared by its packet processor and executor.
use super::{Event, ingress};
use std::{
    cell::Cell,
    marker::PhantomData,
    sync::{Arc, Mutex},
};
use sunrise_mod_runtime::byte_channel::{self, message_overhead};

const RECEIVE_BUDGET: usize = 96 << 20;
// A remaining byte count, or None once this physical connection is retired.
type ReceiveWindow = Mutex<Option<usize>>;

/// Application-lifetime client ingress. A connection borrows a producer, not the drain.
pub struct ClientInbox {
    sender: Arc<byte_channel::Sender<QueuedEvent>>,
    receiver: Option<byte_channel::Receiver<QueuedEvent>>,
    // As with the other ingress consumers, only one actor owns receiving turns.
    _single: PhantomData<Cell<()>>,
}

/// A physical connection's producer and receive window, independent of the FIFO's lifetime.
#[derive(Clone)]
pub struct EventSender {
    serial: u64,
    window: Arc<ReceiveWindow>,
    sender: Arc<byte_channel::Sender<QueuedEvent>>,
}

/// A receiving turn item. Tasks belong to the application, not a connection.
pub struct QueuedEvent {
    pub serial: Option<u64>,
    pub event: Event,
    credit: Option<ClientReceiveCredit>,
}

impl QueuedEvent {
    /// Separate the source, value and credit without refunding the reservation.
    pub fn into_parts(self) -> (Option<u64>, Event, Option<ClientReceiveCredit>) {
        (self.serial, self.event, self.credit)
    }
}

/// Receive credit retained through the client backlog and the chunk decoder.
pub struct ClientReceiveCredit {
    window: Arc<ReceiveWindow>,
    charge: usize,
}

impl Drop for ClientReceiveCredit {
    fn drop(&mut self) {
        if let Some(remaining) = self.window.lock().unwrap().as_mut() {
            *remaining += self.charge;
        }
    }
}

impl Default for ClientInbox {
    fn default() -> Self {
        // Only the physical connection's window charges packet storage. This
        // channel supplies ordering and receiver lifetime, not a second budget.
        let (sender, receiver) = byte_channel::channel(message_overhead::<QueuedEvent>())
            .expect("capacity for an empty event");
        Self {
            sender: Arc::new(sender),
            receiver: Some(receiver),
            _single: PhantomData,
        }
    }
}

impl ClientInbox {
    pub(super) fn connection(&self, serial: u64) -> EventSender {
        EventSender {
            serial,
            window: Arc::new(Mutex::new(Some(RECEIVE_BUDGET))),
            sender: self.sender.clone(),
        }
    }

    /// Take the events already published for this turn, with their original credit.
    #[sunrise_mod_api_macros::replaceable]
    pub fn poll(&self) -> Vec<QueuedEvent> {
        let Some(receiver) = &self.receiver else {
            return Vec::new();
        };
        let count = receiver.stats().queued_items;
        (0..count).map_while(|_| receiver.try_recv().ok()).collect()
    }

    /// Take tasks published before the executor phase, up to the first packet
    /// still waiting for the active connection's next packet drain.
    pub fn poll_ready_tasks(&self, serial: Option<u64>) -> impl Iterator<Item = ClientTask> + '_ {
        self.receiver.iter().flat_map(move |receiver| {
            let count = receiver.stats().queued_items;
            (0..count)
                .map_while(move |_| {
                    receiver.try_recv_if(|item| {
                        matches!(item.event, Event::ClientTask(_)) || item.serial != serial
                    })
                })
                .filter_map(|item| match item.event {
                    Event::ClientTask(task) => Some(task),
                    _ => None, // A packet from a retired connection.
                })
        })
    }

    pub fn close(&mut self) {
        self.receiver = None;
    }

    pub fn is_open(&self) -> bool {
        self.receiver.is_some()
    }

    pub fn executor(&self, owner: &'static str) -> Option<mod_api::ClientExecutor> {
        self.receiver.as_ref()?;
        let sender = self.sender.clone();
        Some(mod_api::ClientExecutor::new(move |run| {
            let _ = sender.send_uncharged(QueuedEvent {
                serial: None,
                event: Event::ClientTask(ClientTask {
                    owner,
                    run: Some(run),
                }),
                credit: None,
            });
        }))
    }

    #[cfg(test)]
    pub(super) fn into_receiver(self) -> byte_channel::Receiver<QueuedEvent> {
        self.receiver.unwrap()
    }

    #[cfg(test)]
    pub(super) fn receiver_for_test(&self) -> &byte_channel::Receiver<QueuedEvent> {
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

pub(crate) struct PreparedEvent {
    event: Event,
    credit: ClientReceiveCredit,
    sender: EventSender,
}

impl PreparedEvent {
    pub(crate) fn publish(self) -> Result<(), ingress::Refused<QueuedEvent>> {
        let value = QueuedEvent {
            serial: Some(self.sender.serial),
            event: self.event,
            credit: Some(self.credit),
        };
        // Serialize retirement and publication. Failed events (and their
        // credit or mod captures) leave both locks before they can be dropped.
        let published = {
            let window = self.sender.window.lock().unwrap();
            if window.is_some() {
                self.sender.sender.send_uncharged(value)
            } else {
                Err(value)
            }
        };
        published.map_err(|value| ingress::Refused {
            reason: ingress::Refusal::Closed,
            value,
        })
    }
}

impl EventSender {
    pub(super) fn close(&self) {
        *self.window.lock().unwrap() = None;
    }

    pub(crate) fn prepare(
        &self,
        event: Event,
    ) -> Result<PreparedEvent, ingress::Refused<QueuedEvent>> {
        let charge = event
            .ingress_charge()
            .saturating_add(message_overhead::<QueuedEvent>());
        let reserved = match self.window.lock().unwrap().as_mut() {
            Some(remaining) if self.sender.receiver_is_open() => {
                match remaining.checked_sub(charge) {
                    Some(next) => {
                        *remaining = next;
                        Ok(())
                    }
                    None => Err(ingress::Refusal::Pressure),
                }
            }
            _ => Err(ingress::Refusal::Closed),
        };
        match reserved {
            Ok(()) => Ok(PreparedEvent {
                event,
                credit: ClientReceiveCredit {
                    window: self.window.clone(),
                    charge,
                },
                sender: self.clone(),
            }),
            Err(reason) => Err(ingress::Refused {
                reason,
                value: QueuedEvent {
                    serial: Some(self.serial),
                    event,
                    credit: None,
                },
            }),
        }
    }

    pub(super) fn send(&self, event: Event) -> Result<(), ingress::Refused<QueuedEvent>> {
        self.prepare(event)?.publish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_phase_leaves_tasks_behind_unhandled_packets() {
        let inbox = ClientInbox::default();
        let connection = inbox.connection(1);
        inbox.executor("before").unwrap().execute(Box::new(|_| {}));
        connection.send(Event::Status("packet".into())).unwrap();
        inbox.executor("after").unwrap().execute(Box::new(|_| {}));

        let owners: Vec<_> = inbox
            .poll_ready_tasks(Some(1))
            .map(|task| task.owner)
            .collect();
        assert_eq!(owners, ["before"]);
        let mut pending = inbox.poll().into_iter();
        assert!(
            matches!(pending.next().unwrap().event, Event::Status(status) if status == "packet")
        );
        assert!(
            matches!(pending.next().unwrap().event, Event::ClientTask(task) if task.owner == "after")
        );
        assert!(pending.next().is_none());
    }

    #[test]
    fn full_retired_connection_cannot_charge_or_publish_into_its_replacement() {
        let inbox = ClientInbox::default();
        let old = inbox.connection(1);
        let status = || Event::Status(String::new());
        // Fill the actual budget with spare capacity, without initializing a large buffer.
        let charge = status().ingress_charge() + message_overhead::<QueuedEvent>();
        old.send(Event::Status(String::with_capacity(
            RECEIVE_BUDGET - 4 * charge,
        )))
        .unwrap();
        let held = inbox.poll(); // retention in the actor backlog or decoder
        old.send(Event::Status(String::with_capacity(charge)))
            .unwrap(); // still in the FIFO
        let pending = old.prepare(status()).unwrap();
        assert_eq!(
            old.send(status()).unwrap_err().reason,
            ingress::Refusal::Pressure
        );

        old.close();
        let new = inbox.connection(2);
        new.send(status()).unwrap(); // no intervening poll or receipt drop
        assert_eq!(
            pending.publish().unwrap_err().reason,
            ingress::Refusal::Closed
        );
        assert_eq!(inbox.poll().last().unwrap().serial, Some(2));
        drop(held);
    }
}
