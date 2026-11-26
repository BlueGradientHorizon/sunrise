//! A receiving main provider reserves capacity before enqueue_work reports Queued.
use super::{ClientEvent, Delivery, Endpoint, Incoming, NoClientEvent};
use crate::{
    lan::NetEvent,
    net_ingress::{Prepared, Producer, ReceiveLease, Refusal, Refused},
};
use mod_api::PayloadContextError;

pub enum MainQueue<E = NoClientEvent> {
    Client(Producer<E>),
    ClientBundle(BundleWork),
    Server {
        events: Producer<NetEvent>,
        endpoint: Endpoint<NoClientEvent>,
        conn: Option<u32>,
    },
}
type BundleWork = std::sync::Arc<std::sync::Mutex<Vec<Incoming>>>;

pub(crate) enum PreparedMain<E = NoClientEvent> {
    Client(Prepared<E>),
    ClientBundle(Box<(BundleWork, Incoming)>),
    Server(Prepared<NetEvent>),
}
/// Keeps failed opaque work alive until after the caller releases its epoch gate.
/// Boxing occurs only on failed publication, adding no allocation to successful publication.
pub(crate) enum FailedMain<E = NoClientEvent> {
    Client(Box<Refused<E>>),
    Server(Box<Refused<NetEvent>>),
}
impl<E> FailedMain<E> {
    pub fn reason(&self) -> Refusal {
        match self {
            Self::Client(error) => error.reason,
            Self::Server(error) => error.reason,
        }
    }
}
impl<E: ClientEvent> MainQueue<E> {
    pub(crate) fn prepare(
        &self,
        incoming: Incoming,
        credit: Option<ReceiveLease<E>>,
    ) -> Result<PreparedMain<E>, PayloadContextError> {
        let refusal = |reason| match reason {
            Refusal::Pressure => PayloadContextError::Backpressure,
            Refusal::Closed => PayloadContextError::Disconnected,
        };
        match self {
            Self::Client(events) => {
                let event = E::raw_payload(incoming);
                match credit {
                    Some(credit) => events.prepare_replacing(event, credit),
                    None => events.prepare(event),
                }
                .map(PreparedMain::Client)
                .map_err(|error| refusal(error.reason))
            }
            Self::ClientBundle(work) => {
                // Incoming (including enqueued work) already holds its payload
                // lease. This root stays inside the one charged bundle turn.
                Ok(PreparedMain::ClientBundle(Box::new((
                    work.clone(),
                    incoming,
                ))))
            }
            Self::Server {
                events,
                endpoint,
                conn,
            } => events
                .prepare(NetEvent::RawPayload(Box::new(Delivery {
                    conn: *conn,
                    endpoint: endpoint.clone(),
                    incoming,
                })))
                .map(PreparedMain::Server)
                .map_err(|error| refusal(error.reason)),
        }
    }
}
impl<E> PreparedMain<E> {
    pub(crate) fn publish(self) -> Result<(), FailedMain<E>> {
        match self {
            Self::Client(prepared) => prepared
                .publish()
                .map_err(|error| FailedMain::Client(Box::new(error))),
            Self::ClientBundle(work) => {
                let (pending, incoming) = *work;
                pending.lock().unwrap().push(incoming);
                Ok(())
            }
            Self::Server(prepared) => prepared
                .publish()
                .map_err(|error| FailedMain::Server(Box::new(error))),
        }
    }
}
