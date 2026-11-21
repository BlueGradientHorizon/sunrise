use super::{tests::empty_item_data, *};
use crate::raw_payloads::{Endpoint, Side, Transport, connection::Epoch};
use mod_api::{
    HandlerThread, Hooks, IPayloadContext, IPayloadHandler, ModEntry, PayloadAccepted,
    PayloadBytes, PayloadContextError, PayloadDirection, PayloadHandleError, PayloadId,
    PayloadProtocol, PayloadSendError, Registrar,
};
use sunrise_protocol::{buf::Writer, connection::Pipe};

fn play_reader(
    mut reader: ConnReader,
    writer: Arc<Mutex<Option<QueuedWriter>>>,
    send: ingress::Producer<Event>,
    data: Arc<ItemData>,
    transport: Arc<Transport>,
) -> JoinHandle<Result<Reason, Reason>> {
    std::thread::spawn(move || {
        receive_play(
            &mut reader,
            &writer,
            &Arc::new(AtomicBool::new(false)),
            &Language::new(),
            &send,
            &Cookies::default(),
            &crate::lod_net::Inbox::default(),
            &data,
            &Arc::new(Mutex::new(None)),
            &transport,
        )
    })
}

#[test]
fn malformed_metadata_aborts_the_open_bundle_and_refunds_its_prefix() {
    let (root, data) = empty_item_data("malformed-bundle");
    let (client, server) = Pipe::pair();
    let (reader, writer) = Connection::local(client).split();
    let writer = Arc::new(Mutex::new(Some(QueuedWriter::new(writer).unwrap())));
    let (send, events) = event_channel();
    let mut server = Connection::local(server);
    let delimiter = play::Clientbound::BundleDelimiter(Empty).encode();
    server.send(&delimiter).unwrap();
    server
        .send(
            &play::Clientbound::AddEntity(play::AddEntity {
                id: 37,
                uuid: 37,
                kind: 0,
                position: [0.5, 64.0, 0.5],
                velocity: [0.0; 3],
                pitch: 0,
                yaw: 0,
                head_yaw: 0,
                data: 0,
            })
            .encode(),
        )
        .unwrap();
    let mut metadata = Writer::new();
    metadata.varint(sunrise_protocol::ids::play::clientbound::SET_ENTITY_DATA);
    metadata.varint(37); // No metadata entries or terminator.
    server.send(&metadata.data).unwrap();
    server.send(&delimiter).unwrap();
    // EOF also lets a receiver that incorrectly skips the decode failure finish the test.
    drop(server);
    let error = play_reader(
        reader,
        writer.clone(),
        send.clone(),
        data,
        Transport::empty(),
    )
    .join()
    .unwrap()
    .unwrap_err();
    assert!(
        error.text.contains("set_entity_data in play bundle"),
        "{error:?}"
    );
    assert!(
        events.try_recv_owned().is_err(),
        "neither the spawn nor a standalone DecodeError may escape"
    );
    assert_eq!(send.stats().reserved_bytes, 0);
    assert!(send.stats().peak_reserved_bytes > 0);
    writer.lock().unwrap().take().unwrap().shutdown();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn malformed_attachment_aborts_packet_decoding_with_or_without_a_bundle() {
    use sunrise_protocol::{attachment_sync, general_entity};

    for bundled in [false, true] {
        assert_malformed_attachment_refused(bundled);
    }

    fn assert_malformed_attachment_refused(bundled: bool) {
        let (root, data) = empty_item_data("malformed-pairing-attachment");
        let transport = Transport::capture(Side::Client).unwrap();
        transport
            .accept(transport.local.manifest.entries())
            .unwrap();
        transport.finish_configuration().unwrap();
        let (client, server) = Pipe::pair();
        let (reader, writer) = Connection::local(client).split();
        let writer = Arc::new(Mutex::new(Some(QueuedWriter::new(writer).unwrap())));
        let (send, events) = event_channel();
        let mut server = Connection::local(server);
        let delimiter = play::Clientbound::BundleDelimiter(Empty);
        let spawn = play::Clientbound::CustomPayload(cfg::CustomPayload {
            channel: general_entity::CHANNEL.into(),
            data: general_entity::encode(&general_entity::Message::Spawn {
                id: 37,
                uuid: 37,
                type_key: "test:general".into(),
                position: [0.5, 64.0, 0.5],
                rotation: [0.0; 2],
                velocity: [0.0; 3],
            })
            .unwrap(),
        });
        let broken = play::Clientbound::CustomPayload(cfg::CustomPayload {
            channel: attachment_sync::CHANNEL.into(),
            data: vec![5, 0, 0], // Invalid holder kind, but a valid outer CustomPayload.
        });
        if bundled {
            server.send(&delimiter.encode()).unwrap();
            server.send(&spawn.encode()).unwrap();
        }
        server.send(&broken.encode()).unwrap();
        if bundled {
            server.send(&delimiter.encode()).unwrap();
        }
        drop(server);
        let reader = play_reader(reader, writer.clone(), send, data, transport);
        assert!(
            matches!(
                events.recv_timeout_owned(Duration::from_secs(5)),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
            ),
            "the reader must close without publishing the spawn or its attachments"
        );
        let error = reader.join().unwrap().unwrap_err();
        assert_eq!(
            error.text,
            "attachment sync in play: attachment sync: holder kind 5"
        );
        assert_eq!(events.stats().reserved_bytes, 0);
        assert_eq!(events.stats().peak_reserved_bytes > 0, bundled);
        writer.lock().unwrap().take().unwrap().shutdown();
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn a_pairing_bundle_accepts_a_wire_valid_high_expansion_attachment() {
    use sunrise_core::nbt::Tag;
    use sunrise_mod_runtime::attachments::Holder;
    use sunrise_protocol::attachment_sync;

    let (root, data) = empty_item_data("expanded-pairing-attachment");
    let transport = Transport::capture(Side::Client).unwrap();
    transport
        .accept(transport.local.manifest.entries())
        .unwrap();
    transport.finish_configuration().unwrap();
    let (client, server) = Pipe::pair();
    let (reader, writer) = Connection::local(client).split();
    let writer = Arc::new(Mutex::new(Some(QueuedWriter::new(writer).unwrap())));
    let (send, events) = event_channel();
    let mut server = Connection::local(server);
    let delimiter = play::Clientbound::BundleDelimiter(Empty).encode();
    let spawn = play::Clientbound::AddEntity(play::AddEntity {
        id: 37,
        uuid: 37,
        kind: 0,
        position: [0.5, 64.0, 0.5],
        velocity: [0.0; 3],
        pitch: 0,
        yaw: 0,
        head_yaw: 0,
        data: 0,
    });
    // Entity 37, replace, one change a:b. Its nameless compound has a list of
    // 200,000 byte compounds (each {v: -7}), about 1.2 MB on the wire.
    const COUNT: u32 = 200_000;
    let mut body = vec![0, 37, 1, 1, 1, b'a', 1, b'b', 10, 9, 0, 1, b'l', 10];
    body.extend(COUNT.to_be_bytes());
    for _ in 0..COUNT {
        body.extend([1, 0, 1, b'v', 249, 0]);
    }
    body.push(0);
    let attachments = play::Clientbound::CustomPayload(cfg::CustomPayload {
        channel: attachment_sync::CHANNEL.into(),
        data: body,
    });
    let reader = play_reader(reader, writer.clone(), send, data, transport);
    for packet in [
        delimiter.clone(),
        spawn.encode(),
        attachments.encode(),
        delimiter,
    ] {
        server.send(&packet).unwrap();
    }
    drop(server);
    let timeout = Duration::from_secs(5);
    let take = || events.recv_timeout_owned(timeout).unwrap().into_parts().0;
    assert!(matches!(take(), Event::BundleStart(None)));
    assert!(matches!(take(), Event::Packet(p) if *p == spawn));
    let Event::AttachmentSync { payload, message } = take() else {
        panic!("expected a decoded attachment sync");
    };
    assert_eq!(play::Clientbound::CustomPayload(payload), attachments);
    assert!(matches!(take(), Event::BundleEnd));
    let message = message.into_message();
    assert_eq!(message.holder, Holder::Entity(37));
    assert!(message.replace);
    let [change] = &message.changes[..] else {
        panic!("expected one attachment change");
    };
    assert_eq!(change.namespace, JavaString::from("a"));
    assert_eq!(change.name, JavaString::from("b"));
    let Some(Tag::List(items)) = change.value.as_ref().and_then(|map| map.get(&"l".into())) else {
        panic!("expected the staged attachment list");
    };
    let key = "v".into();
    assert_eq!(items.len(), COUNT as usize);
    assert!(
        items.iter().all(
            |item| matches!(item, Tag::Compound(map) if map.get(&key) == Some(&Tag::Byte(-7)))
        )
    );
    assert_eq!(events.stats().reserved_bytes, 0);
    reader.join().unwrap().unwrap_err();
    writer.lock().unwrap().take().unwrap().shutdown();
    std::fs::remove_dir_all(root).unwrap();
}

const MAIN: &str = "bundle_order_probe:main";
const NETWORK: &str = "bundle_order_probe:network";
struct Probe(Arc<Mutex<Vec<(u8, std::thread::ThreadId)>>>);
impl IPayloadHandler for Probe {
    fn handle(
        &self,
        bytes: PayloadBytes,
        ctx: &mut dyn IPayloadContext,
    ) -> Result<(), PayloadHandleError> {
        let step = bytes.0[0];
        self.0
            .lock()
            .unwrap()
            .push((step, std::thread::current().id()));
        if step == 4 {
            ctx.disconnect(mod_api::Text::literal("bundle disconnect"));
        } else {
            let channel = if step == 1 { MAIN } else { NETWORK };
            if step == 3 {
                assert!(matches!(
                    ctx.client_host(),
                    Err(PayloadContextError::WrongThread)
                ));
            }
            ctx.reply(&PayloadId(channel.into()), bytes).unwrap();
        }
        Ok(())
    }
}
struct Registration(Arc<Probe>);
#[mod_api::__subscriptions]
impl Hooks for Registration {
    fn register(&self, registrar: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        let mut handlers = registrar.payload_handlers();
        let mut payloads = handlers.registrar("1");
        payloads.play_bidirectional(PayloadId(MAIN.into()), self.0.clone(), Some(self.0.clone()));
        payloads
            .executes_on(HandlerThread::Network)
            .play_bidirectional(
                PayloadId(NETWORK.into()),
                self.0.clone(),
                Some(self.0.clone()),
            );
    }
}

// Like Game's main root, this test sink publishes real endpoint replies before
// returning to the bundle, without loading game resources or constructing a Game.
struct MainReply<'a>(&'a Endpoint, &'a Arc<Epoch>);
impl IPayloadContext for MainReply<'_> {
    fn player(&self) -> Result<mod_api::PlayerRef, PayloadContextError> {
        unreachable!()
    }
    fn flow(&self) -> PayloadDirection {
        PayloadDirection::ToClient
    }
    fn protocol(&self) -> PayloadProtocol {
        PayloadProtocol::Play
    }
    fn server_host(&mut self) -> Result<&mut dyn mod_api::ServerHost, PayloadContextError> {
        unreachable!()
    }
    fn client_host(&mut self) -> Result<&mut dyn mod_api::ClientHost, PayloadContextError> {
        unreachable!()
    }
    fn reply(
        &mut self,
        id: &PayloadId,
        bytes: PayloadBytes,
    ) -> Result<PayloadAccepted, PayloadSendError> {
        let reply = self.0.prepare(self.1, id, bytes)?;
        self.1
            .with_active(|| reply.publish())
            .ok_or(PayloadSendError::StaleEpoch)??;
        Ok(PayloadAccepted::Queued)
    }
    fn check_payload_token(&self, _: &mod_api::PayloadToken) -> Result<(), PayloadSendError> {
        unreachable!()
    }
    fn send_to(
        &mut self,
        _: &[mod_api::PlayerRef],
        _: &PayloadId,
        _: PayloadBytes,
    ) -> Result<u32, PayloadSendError> {
        unreachable!()
    }
    fn finish_configuration_task(
        &mut self,
        _: mod_api::ConfigurationTaskRef,
    ) -> Result<(), mod_api::ConfigurationTaskError> {
        unreachable!()
    }
    fn disconnect(&mut self, _: mod_api::Text) {
        unreachable!()
    }
    fn enqueue_work(
        &mut self,
        _: Arc<dyn mod_api::PayloadWork>,
    ) -> Result<mod_api::PayloadWorkStatus, PayloadContextError> {
        unreachable!()
    }
}

#[test]
fn inline_main_receipt_pressure_closes_the_epoch_at_the_routed_step() {
    mod_platform::__private::initialize().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let registration = Registration(Arc::new(Probe(seen.clone())));
    mod_platform::testing::with_mods(
        vec![ModEntry::new("bundle_order_probe", Box::new(registration))].into(),
        || {
            mod_platform::__private::part_tests::install_parts_fixture(mod_api::__private::mods())
                .unwrap();
            let transport = Transport::capture(Side::Client).unwrap();
            transport
                .accept(transport.local.manifest.entries())
                .unwrap();
            transport.finish_configuration().unwrap();
            let epoch = transport.epoch().unwrap();
            // The test producer's allowance admits the packet, but can't hold
            // the Main envelope. Both deliveries must use this queue's charge.
            const BUDGET: usize = 16 << 10;
            let (send, events) = ingress::channel(BUDGET, |event| match event {
                Event::RawPayload(_) => BUDGET,
                _ => event.ingress_charge(),
            })
            .unwrap();
            send.send(Event::Packet(Box::new(play::Clientbound::CustomPayload(
                cfg::CustomPayload {
                    channel: MAIN.into(),
                    data: vec![1],
                },
            ))))
            .unwrap();
            let (Event::Packet(packet), credit) = events.try_recv_owned().unwrap().into_parts()
            else {
                panic!("expected a staged packet");
            };
            let writer = Arc::new(Mutex::new(None));
            let endpoint = Endpoint::client(
                writer.clone(),
                Arc::new(AtomicBool::new(false)),
                transport.clone(),
                send.clone(),
            );
            let mut router = PlayRouter {
                writer: &writer,
                lang: &Language::new(),
                send: &send,
                cookies: &Cookies::default(),
                far: &crate::lod_net::Inbox::default(),
                endpoint,
                batch: Default::default(),
            };
            let mut step = BundleStep {
                event: None,
                inline: Vec::new(),
                stop: false,
                credit: Some(credit),
            };
            assert!(
                router.route(*packet, Some(&mut step)).is_err(),
                "Main must refuse under ingress pressure"
            );
            assert!(transport.closed());
            assert!(!epoch.active());
            assert!(step.inline.is_empty());
            assert!(seen.lock().unwrap().is_empty());
            assert_eq!(events.stats().reserved_bytes, 0);
        },
    );
}

#[test]
fn mixed_bundle_executes_main_and_network_in_order_before_disconnect() {
    mod_platform::__private::initialize().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let registration = Registration(Arc::new(Probe(seen.clone())));
    mod_platform::testing::with_mods(
        vec![ModEntry::new("bundle_order_probe", Box::new(registration))].into(),
        || {
            mod_platform::__private::part_tests::install_parts_fixture(mod_api::__private::mods())
                .unwrap();
            let transport = Transport::capture(Side::Client).unwrap();
            transport
                .accept(transport.local.manifest.entries())
                .unwrap();
            transport.finish_configuration().unwrap();
            let (root, data) = empty_item_data("mixed-bundle");
            let (client, server) = Pipe::pair();
            let (reader, writer) = Connection::local(client).split();
            let (mut server_reader, mut server_writer) = Connection::local(server).split();
            let timeout = Duration::from_secs(5);
            server_reader.set_read_timeout(Some(timeout)).unwrap();
            let writer = Arc::new(Mutex::new(Some(QueuedWriter::new(writer).unwrap())));
            let delimiter = play::Clientbound::BundleDelimiter(Empty);
            let custom = |channel: &str, step, bytes| {
                play::Clientbound::CustomPayload(cfg::CustomPayload {
                    channel: channel.into(),
                    data: vec![step; bytes],
                })
            };
            let subpackets = [
                play::Clientbound::Ping(cfg::Ping { id: 0 }),
                custom(MAIN, 1, 8 * 1024),
                play::Clientbound::Ping(cfg::Ping { id: 2 }),
                custom(NETWORK, 3, 1),
                custom(NETWORK, 4, 1),
                play::Clientbound::Ping(cfg::Ping { id: 5 }),
            ];
            let budget = subpackets
                .iter()
                .map(|packet| Event::Packet(Box::new(packet.clone())).ingress_charge())
                .sum::<usize>()
                + Event::BundleStart(None).ingress_charge()
                + Event::BundleEnd.ingress_charge()
                + (subpackets.len() + 2)
                    * sunrise_mod_runtime::byte_channel::message_overhead::<Event>()
                + 1024; // Room for a different envelope, but not another 8 KiB payload.
            let (send, events) = ingress::channel(budget, Event::ingress_charge).unwrap();
            let endpoint = Endpoint::client(
                writer.clone(),
                Arc::new(AtomicBool::new(false)),
                transport.clone(),
                send.clone(),
            );
            let reader = play_reader(reader, writer.clone(), send, data, transport.clone());
            let network_thread = reader.thread().id();
            let main_thread = std::thread::current().id();
            server_writer.send(&delimiter.encode()).unwrap();
            for packet in subpackets {
                server_writer.send(&packet.encode()).unwrap();
            }
            server_writer.send(&delimiter.encode()).unwrap();
            let (start, _start_credit) = events.recv_timeout_owned(timeout).unwrap().into_parts();
            let Event::BundleStart(Some(dispatch)) = start else {
                panic!("expected routed bundle");
            };
            assert!(
                seen.lock().unwrap().is_empty(),
                "staging must not execute callbacks"
            );
            let mut stopped = false;
            for receipt in events.take_turn() {
                let (event, credit) = receipt.into_parts();
                if matches!(event, Event::BundleEnd) {
                    break;
                }
                if stopped {
                    continue;
                }
                let progress = dispatch
                    .dispatch(event, credit, |event| {
                        match event {
                            Event::Packet(packet) => {
                                let play::Clientbound::Ping(ping) = *packet else {
                                    panic!("expected ping");
                                };
                                seen.lock().unwrap().push((ping.id as u8, main_thread));
                            }
                            Event::RawPayload(incoming) => {
                                assert_eq!(incoming.receiver.thread, HandlerThread::Main);
                                assert!(incoming.epoch.active());
                                let crate::raw_payloads::Body::Payload(bytes) = incoming.body
                                else {
                                    panic!("expected Main payload");
                                };
                                incoming
                                    .receiver
                                    .handle(bytes, &mut MainReply(&endpoint, &incoming.epoch))
                                    .unwrap();
                            }
                            _ => panic!("unexpected bundle step"),
                        }
                        Ok::<_, std::convert::Infallible>(false)
                    })
                    .unwrap();
                stopped = matches!(progress, BundleProgress::Stop);
            }
            drop(dispatch);
            assert!(stopped);
            assert!(transport.closed());
            assert_eq!(
                *seen.lock().unwrap(),
                [
                    (0, main_thread),
                    (1, main_thread),
                    (2, main_thread),
                    (3, network_thread),
                    (4, network_thread)
                ]
            );
            for (channel, marker, bytes) in [(MAIN, 1, 8 * 1024), (NETWORK, 3, 1)] {
                let (id, payload) = server_reader.recv().unwrap();
                let play::Serverbound::CustomPayload(reply) =
                    play::Serverbound::decode(id, &payload).unwrap()
                else {
                    panic!("expected a reply");
                };
                assert_eq!(reply.channel, channel);
                assert_eq!(reply.data, vec![marker; bytes]);
            }
            assert_eq!(
                reader.join().unwrap().unwrap_err().text,
                "bundle disconnect"
            );
            writer.lock().unwrap().take().unwrap().shutdown();
            std::fs::remove_dir_all(root).unwrap();
        },
    );
}
