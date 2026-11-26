//! Physical reply targets usable by either receiving executor without borrowing Game.
use super::{
    ClientEvent, NoClientEvent, Side, Transport,
    connection::Epoch,
    encode, encoded_len,
    queued_writer::{PreparedPayload, QueuedWriter},
};
use crate::{
    lan::{PlayerSender, config_payloads::ConfigSender},
    net_ingress::Producer,
};
use mod_api::{PayloadBytes, PayloadFrameStage, PayloadId, PayloadSendError, Text};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sunrise_protocol::{connection::Framing, frame, payload_negotiation::Phase};

pub struct Endpoint<E = NoClientEvent>(Target<E>);
impl<E> Clone for Endpoint<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
enum Target<E> {
    Client {
        writer: Arc<Mutex<Option<QueuedWriter>>>,
        configuring: Arc<AtomicBool>,
        transport: Arc<Transport>,
        events: Producer<E>,
    },
    Server(PlayerSender),
    Configuration {
        sender: ConfigSender,
        session: Option<std::sync::Weak<crate::lan::configuration_tasks::Session>>,
    },
}
impl<E> Clone for Target<E> {
    fn clone(&self) -> Self {
        match self {
            Self::Client {
                writer,
                configuring,
                transport,
                events,
            } => Self::Client {
                writer: writer.clone(),
                configuring: configuring.clone(),
                transport: transport.clone(),
                events: events.clone(),
            },
            Self::Server(sender) => Self::Server(sender.clone()),
            Self::Configuration { sender, session } => Self::Configuration {
                sender: sender.clone(),
                session: session.clone(),
            },
        }
    }
}
/// Retains no endpoint, sender, producer, writer, transport or payload lease.
#[cfg(any(test, feature = "test-payload-probe"))]
pub(super) struct Observation {
    transport: std::sync::Weak<Transport>,
    side: Side,
    diagnostics: Box<dyn Fn() -> String + Send + Sync>,
}
#[cfg(any(test, feature = "test-payload-probe"))]
impl Observation {
    pub(super) fn same_target<E>(&self, endpoint: &Endpoint<E>) -> bool {
        self.transport.ptr_eq(&Arc::downgrade(endpoint.transport())) && self.side == endpoint.side()
    }
    pub(super) fn diagnostics(&self) -> String {
        let connection = match self.transport.upgrade() {
            Some(transport) => transport
                .connection
                .as_ref()
                .map(|c| c.diagnostics())
                .unwrap_or_else(|| "no connection".into()),
            None => "transport owner dropped".into(),
        };
        format!("{:?}: {connection}; {}", self.side, (self.diagnostics)())
    }
}
pub enum Prepared {
    Client(PreparedPayload),
    Server(crate::lan::conn::PreparedPayload),
    /// A `Droppable` send: it reserves nothing, and publication queues it for the writer.
    Droppable(
        Arc<super::droppable::Queue>,
        Arc<mod_api::__private::game::DropCounters>,
        Vec<u8>,
    ),
}
impl Prepared {
    pub fn publish(self) -> Result<(), PayloadSendError> {
        match self {
            Self::Client(send) => send.publish(),
            Self::Server(send) => send.publish(),
            Self::Droppable(queue, counters, body) => {
                queue.push(std::time::Instant::now(), &counters, body);
                Ok(())
            }
        }
    }
    /// Extract a server `PreparedPayload` as a `Body::Prepared` for inclusion inside
    /// an entity pairing bundle.
    pub(crate) fn into_prepared_body(self, epoch: Arc<Epoch>) -> Option<crate::lan::Body> {
        match self {
            Self::Server(prepared) => Some(prepared.into_prepared_body(epoch)),
            Self::Client(_) | Self::Droppable(..) => None,
        }
    }
}
impl Endpoint<NoClientEvent> {
    pub fn server(sender: PlayerSender) -> Self {
        Self(Target::Server(sender))
    }
    pub fn configuration(
        sender: ConfigSender,
        session: Option<std::sync::Weak<crate::lan::configuration_tasks::Session>>,
    ) -> Self {
        Self(Target::Configuration { sender, session })
    }
}
impl<E: ClientEvent> Endpoint<E> {
    pub fn client(
        writer: Arc<Mutex<Option<QueuedWriter>>>,
        configuring: Arc<AtomicBool>,
        transport: Arc<Transport>,
        events: Producer<E>,
    ) -> Self {
        Self(Target::Client {
            writer,
            configuring,
            transport,
            events,
        })
    }
}
impl<E> Endpoint<E> {
    pub fn configuration_session(
        &self,
    ) -> Option<&std::sync::Weak<crate::lan::configuration_tasks::Session>> {
        match &self.0 {
            Target::Configuration { session, .. } => session.as_ref(),
            _ => None,
        }
    }
    pub fn side(&self) -> Side {
        match self.0 {
            Target::Client { .. } => Side::Client,
            _ => Side::Server,
        }
    }
    pub fn transport(&self) -> &Arc<Transport> {
        match &self.0 {
            Target::Client { transport, .. } => transport,
            Target::Server(sender) => sender.payload_transport(),
            Target::Configuration { sender, .. } => sender.payload_transport(),
        }
    }
    #[cfg(any(test, feature = "test-payload-probe"))]
    pub(super) fn observation(&self) -> Observation {
        let diagnostics: Box<dyn Fn() -> String + Send + Sync> = match &self.0 {
            Target::Client { writer, .. } => {
                let writer = Arc::downgrade(writer);
                Box::new(move || {
                    let Some(writer) = writer.upgrade() else {
                        return "writer handle owner dropped".into();
                    };
                    match writer.try_lock() {
                        Ok(w) => {
                            let current = w.clone();
                            drop(w);
                            current
                                .map(|w| w.diagnostics())
                                .unwrap_or_else(|| "writer absent".into())
                        }
                        Err(_) => "writer handle busy/poisoned".into(),
                    }
                })
            }
            Target::Server(sender) => sender.payload_diagnostic_observer(),
            Target::Configuration { .. } => Box::new(|| "configuration sender not observed".into()),
        };
        Observation {
            transport: Arc::downgrade(self.transport()),
            side: self.side(),
            diagnostics,
        }
    }
    /// Peer authority is checked before bytes are copied. Publication belongs to the root.
    pub fn prepare(
        &self,
        epoch: &Arc<Epoch>,
        id: &PayloadId,
        bytes: PayloadBytes,
    ) -> Result<Prepared, PayloadSendError> {
        #[cfg(any(test, feature = "test-payload-probe"))]
        super::staging_probe::capture(&id.0, self);
        epoch.can_send(id)?;
        // A raw channel keeps vanilla's clientbound custom-payload limit, by name.
        let limit = sunrise_protocol::packets::custom_payload::CLIENTBOUND_LIMIT;
        if epoch.peer.local.is_raw(&id.0) && bytes.0.len() > limit {
            return Err(PayloadSendError::PayloadTooLarge {
                id: id.clone(),
                size: bytes.0.len() as u64,
                limit: limit as u64,
            });
        }
        let len = encoded_len(epoch.phase, self.side(), id, bytes.0.len());
        if epoch.peer.local.has_droppable()
            && let Some(counters) = epoch.peer.local.droppable(&id.0)
        {
            return self.prepare_droppable(epoch, id, bytes, len, counters);
        }
        match &self.0 {
            Target::Client {
                writer,
                configuring,
                ..
            } => {
                if configuring.load(Ordering::Acquire) != (epoch.phase == Phase::Configuration) {
                    return Err(PayloadSendError::WrongProtocol(id.clone()));
                }
                let writer = writer
                    .lock()
                    .unwrap()
                    .as_ref()
                    .cloned()
                    .ok_or(PayloadSendError::Disconnected)?;
                preflight(writer.framing(), writer.payload_capacity(), len, id)?;
                writer
                    .prepare_payload(encode(epoch.phase, self.side(), id, bytes), id)
                    .map(Prepared::Client)
            }
            Target::Server(sender) => {
                preflight(sender.payload_framing(), sender.payload_capacity(), len, id)?;
                sender
                    .prepare_payload(encode(epoch.phase, self.side(), id, bytes), id)
                    .map(Prepared::Server)
            }
            Target::Configuration { sender, .. } => {
                preflight(sender.framing(), sender.payload_capacity(), len, id)?;
                sender
                    .prepare_payload(encode(epoch.phase, self.side(), id, bytes), id)
                    .map(Prepared::Client)
            }
        }
    }
    /// A `Droppable` send takes no reservation: it is bounded by its own queue. Frame limits
    /// still refuse here, so no waiting send can fail framing later.
    fn prepare_droppable(
        &self,
        epoch: &Arc<Epoch>,
        id: &PayloadId,
        bytes: PayloadBytes,
        len: usize,
        counters: &Arc<mod_api::__private::game::DropCounters>,
    ) -> Result<Prepared, PayloadSendError> {
        let framing = match &self.0 {
            Target::Client {
                writer,
                configuring,
                ..
            } => {
                if configuring.load(Ordering::Acquire) != (epoch.phase == Phase::Configuration) {
                    return Err(PayloadSendError::WrongProtocol(id.clone()));
                }
                let writer = writer.lock().unwrap();
                writer
                    .as_ref()
                    .filter(|writer| writer.accepts_packets())
                    .ok_or(PayloadSendError::Disconnected)?
                    .framing()
            }
            Target::Server(sender) => sender.payload_framing(),
            Target::Configuration { sender, .. } => sender.framing(),
        };
        preflight(framing, usize::MAX, len, id)?;
        let queue = self
            .transport()
            .connection
            .as_ref()
            .and_then(|connection| connection.droppable())
            .ok_or(PayloadSendError::Disconnected)?;
        Ok(Prepared::Droppable(
            queue.clone(),
            counters.clone(),
            encode(epoch.phase, self.side(), id, bytes),
        ))
    }
}
impl<E: ClientEvent> Endpoint<E> {
    /// A failure closes this physical connection only; a client reason joins its ordered events.
    pub fn disconnect(&self, reason: Text) {
        let reason = crate::api_text::to_serialized_engine(&reason).unwrap_or_else(|error| {
            crate::text::Text::tr(
                "disconnect.genericReason",
                &[&format!("invalid payload disconnect reason: {error:?}")],
            )
        });
        self.transport().close();
        match &self.0 {
            Target::Client {
                writer,
                configuring,
                events,
                ..
            } => {
                let writer = writer.lock().unwrap();
                let _ = events.send_terminal(E::disconnected(reason.plain()));
                if let Some(writer) = writer.as_ref() {
                    if configuring.load(Ordering::Acquire) {
                        writer.shutdown();
                    } else {
                        writer.close_after_pending();
                    }
                }
            }
            Target::Server(sender) => {
                sender.disconnect(crate::lan::data::component_nbt(&reason.component()))
            }
            Target::Configuration { sender, .. } => {
                sender.disconnect(crate::lan::data::component_nbt(&reason.component()))
            }
        }
    }
}
fn preflight(
    framing: Framing,
    capacity: usize,
    len: usize,
    id: &PayloadId,
) -> Result<(), PayloadSendError> {
    let frame_limit = match framing {
        Framing::Memory => None,
        Framing::Network(None) => Some((PayloadFrameStage::Encoded, frame::MAX_FRAME)),
        Framing::Network(Some(_)) => {
            Some((PayloadFrameStage::Uncompressed, frame::MAX_UNCOMPRESSED))
        }
    };
    if let Some((stage, limit)) = frame_limit
        && len > limit
    {
        return Err(PayloadSendError::FrameTooLarge {
            id: id.clone(),
            stage,
            size: len as u64,
            limit: limit as u64,
        });
    }
    if len > capacity {
        return Err(PayloadSendError::Backpressure);
    }
    Ok(())
}
