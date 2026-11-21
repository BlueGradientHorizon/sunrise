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
            let (send, events) = event_channel();
            let endpoint = Endpoint::client(
                writer.clone(),
                Arc::new(AtomicBool::new(false)),
                transport.clone(),
                send.clone(),
            );
            let reader = play_reader(reader, writer.clone(), send, data, transport.clone());
            let network_thread = reader.thread().id();
            let main_thread = std::thread::current().id();
            let delimiter = play::Clientbound::BundleDelimiter(Empty);
            let custom = |channel: &str, step| {
                play::Clientbound::CustomPayload(cfg::CustomPayload {
                    channel: channel.into(),
                    data: vec![step],
                })
            };
            for packet in [
                delimiter.clone(),
                play::Clientbound::Ping(cfg::Ping { id: 0 }),
                custom(MAIN, 1),
                play::Clientbound::Ping(cfg::Ping { id: 2 }),
                custom(NETWORK, 3),
                custom(NETWORK, 4),
                play::Clientbound::Ping(cfg::Ping { id: 5 }),
                delimiter,
            ] {
                server_writer.send(&packet.encode()).unwrap();
            }
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
                let (event, _credit) = receipt.into_parts();
                if matches!(event, Event::BundleEnd) {
                    break;
                }
                if stopped {
                    continue;
                }
                let progress = dispatch
                    .dispatch(event, |event| {
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
            for (channel, marker) in [(MAIN, 1), (NETWORK, 3)] {
                let (id, payload) = server_reader.recv().unwrap();
                let play::Serverbound::CustomPayload(reply) =
                    play::Serverbound::decode(id, &payload).unwrap()
                else {
                    panic!("expected a reply");
                };
                assert_eq!(reply.channel, channel);
                assert_eq!(reply.data, [marker]);
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
