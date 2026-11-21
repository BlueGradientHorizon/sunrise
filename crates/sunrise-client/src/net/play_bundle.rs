//! PacketBundlePacker assembles on receive; ClientPacketListener runs the completed
//! packet in one main turn. The receiver parks until that turn ends, serving only
//! the typed payload callbacks that need its NETWORK executor, in member order.
use super::{Event, PlayIo, PlayIoAction, Reason, brand_event, ingress};
use crate::raw_payloads::{Classified, Endpoint, Incoming, dispatch_bundle};
use std::sync::{Arc, Mutex, mpsc};
use sunrise_protocol::{
    packets::{custom_payload::CustomPayload, play::Clientbound as C},
    payload_negotiation::Phase,
};

#[derive(Default)]
pub(super) struct PlayBundle(Option<Vec<C>>);

pub(super) enum PlayTurn {
    Single(C),
    Bundle(Vec<C>),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum InvalidBundle {
    Terminal,
    TooManyPackets,
}

impl PlayBundle {
    pub(super) fn assembling(&self) -> bool {
        self.0.is_some()
    }

    pub(super) fn accept(&mut self, packet: C) -> Result<Option<PlayTurn>, InvalidBundle> {
        if matches!(packet, C::BundleDelimiter(_)) {
            return Ok(match self.0.take() {
                Some(packets) => Some(PlayTurn::Bundle(packets)),
                None => {
                    self.0 = Some(Vec::new());
                    None
                }
            });
        }
        let Some(packets) = &mut self.0 else {
            return Ok(Some(PlayTurn::Single(packet)));
        };
        if matches!(packet, C::StartConfiguration(_)) {
            return Err(InvalidBundle::Terminal);
        }
        if packets.len() == 4096 {
            return Err(InvalidBundle::TooManyPackets);
        }
        packets.push(packet);
        Ok(None)
    }
}

pub(super) enum BundleEnd {
    Transfer,
    Server(sunrise_core::nbt::Tag),
    Mod(String),
    InvalidPayload,
}

/// Once the close member (or a mod disconnect) is reached, no later root runs.
#[derive(Clone, Default)]
pub(crate) struct BundleClose(Arc<Mutex<Option<BundleEnd>>>);
impl BundleClose {
    fn set(&self, reason: BundleEnd) {
        self.0.lock().unwrap().get_or_insert(reason);
    }

    pub(crate) fn mod_disconnect(&self, reason: String) {
        self.set(BundleEnd::Mod(reason));
    }

    fn requested(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }
}

struct PayloadRequest {
    payload: CustomPayload,
    reply: mpsc::SyncSender<BundleStep>,
}

pub struct BundleDriver {
    io: Arc<PlayIo>,
    close: BundleClose,
    endpoint: Endpoint,
    requests: mpsc::Sender<PayloadRequest>,
}

pub enum BundleStep {
    Event(Event),
    Main(Vec<Incoming>),
    Continue,
    Stop,
}

pub struct CompletedBundle {
    pub packets: Vec<C>,
    pub driver: BundleDriver,
}

impl CompletedBundle {
    pub(super) fn owned_bytes(&self) -> usize {
        self.packets.capacity() * std::mem::size_of::<C>()
            + self.packets.iter().map(C::owned_heap_bytes).sum::<usize>()
            + std::mem::size_of::<Self>()
    }
}

impl BundleDriver {
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn closing(&self) -> bool {
        self.close.requested()
    }

    /// A single member's vanilla effect or main payload root. Only a typed
    /// payload crosses to the receiving executor; ordinary packets stay here.
    pub fn step(&self, packet: C) -> BundleStep {
        let action = match packet {
            C::KeepAlive(p) => PlayIoAction::KeepAlive(p),
            C::ChunkBatchStart(_) => PlayIoAction::ChunkBatchStart,
            C::ChunkBatchFinished(p) => PlayIoAction::ChunkBatchFinished(p.size),
            C::CookieRequest(p) => PlayIoAction::CookieRequest(p),
            C::StoreCookie(p) => PlayIoAction::StoreCookie(p),
            C::ResourcePackPush(p) => return BundleStep::Event(Event::PackPush(p)),
            C::ResourcePackPop(p) => return BundleStep::Event(Event::PackPop(p.id)),
            C::ServerLinks(p) => return BundleStep::Event(Event::ServerLinks(p.links)),
            C::Transfer(p) => {
                self.close.set(BundleEnd::Transfer);
                return BundleStep::Event(Event::Transfer {
                    host: p.host,
                    port: p.port,
                });
            }
            C::Disconnect(p) => {
                self.close.set(BundleEnd::Server(p.reason));
                return BundleStep::Stop;
            }
            C::CustomPayload(p) if p.channel == "minecraft:brand" => {
                return brand_event(&p).map_or(BundleStep::Continue, BundleStep::Event);
            }
            C::CustomPayload(p) if p.channel == crate::lod_net::CHANNEL => {
                PlayIoAction::FarTile(p.data)
            }
            C::CustomPayload(payload) => {
                let (reply, response) = mpsc::sync_channel(0);
                if self
                    .requests
                    .send(PayloadRequest { payload, reply })
                    .is_err()
                {
                    return BundleStep::Stop;
                }
                return response.recv().unwrap_or(BundleStep::Stop);
            }
            other => return BundleStep::Event(Event::Packet(Box::new(other))),
        };
        self.io.handle(action);
        BundleStep::Continue
    }
}

/// The whole count-bounded record has ordinary ingress accounting, irrespective
/// of the watermark. Only one can be queued or running: the receiver waits here
/// until the main driver is dropped. Dropping the session also drops the driver.
pub(super) fn deliver(
    packets: Vec<C>,
    io: Arc<PlayIo>,
    endpoint: Endpoint,
    events: &ingress::Producer<Event>,
) -> Result<Option<BundleEnd>, Reason> {
    let close = BundleClose::default();
    let endpoint = endpoint.in_bundle(close.clone());
    let (requests, receive) = mpsc::channel::<PayloadRequest>();
    events
        .send_ordered(Event::Bundle(Box::new(CompletedBundle {
            packets,
            driver: BundleDriver {
                io,
                close: close.clone(),
                endpoint: endpoint.clone(),
                requests,
            },
        })))
        .map_err(|_| Reason::plain("Disconnected"))?;
    for PayloadRequest { payload, reply } in receive {
        let step = match Incoming::classify(endpoint.transport(), Phase::Play, payload) {
            Ok(Classified::Other(payload)) => {
                BundleStep::Event(Event::Packet(Box::new(C::CustomPayload(payload))))
            }
            Ok(Classified::Application(incoming)) => {
                let work = dispatch_bundle(incoming, &endpoint);
                if close.requested() {
                    BundleStep::Stop
                } else {
                    BundleStep::Main(work)
                }
            }
            Ok(Classified::Raw(_)) => BundleStep::Continue,
            Err(error) => {
                log::warn!("Sunrise: bundle payload refused: {error}");
                close.set(BundleEnd::InvalidPayload);
                BundleStep::Stop
            }
        };
        if reply.send(step).is_err() {
            break;
        }
    }
    let reason = close.0.lock().unwrap().take();
    Ok(reason)
}
