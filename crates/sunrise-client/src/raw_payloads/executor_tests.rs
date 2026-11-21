//! Real network executor/provider tests. No World or substitute payload context.
use super::*;
use crate::{
    lan::{NetEvent, PlayerSender},
    net::ingress::{Consumer, Producer},
};
use mod_api::{
    HandlerThread, Hooks, IPayloadContext, IPayloadHandler, ModEntry, PayloadBytes,
    PayloadContextError, PayloadWork, PayloadWorkStatus, Registrar,
};
use std::sync::{
    Mutex, Weak,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

const CHANNEL: &str = "executor_probe:message";
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Success,
    Pressure,
    Error,
    Panic,
    Cancel,
    Reconfigure,
    PublicationClosed,
}
struct Probe {
    mode: Mode,
    events: Producer<NetEvent>,
    receiver: Mutex<Option<Consumer<NetEvent>>>,
    transport: Mutex<Weak<Transport>>,
    drops: Arc<AtomicUsize>,
    calls: AtomicUsize,
}
struct ReentrantWork {
    epoch: Arc<connection::Epoch>,
    drops: Arc<AtomicUsize>,
}
impl PayloadWork for ReentrantWork {
    fn run(&self, _: &mut dyn IPayloadContext) -> Result<(), String> {
        panic!("network dispatch must not execute queued main work inline")
    }
}
impl Drop for ReentrantWork {
    fn drop(&mut self) {
        // A publication-gate Drop mutant deadlocks here. The bounded worker
        // completion assertion below fails without joining the stuck thread.
        let _ = self.epoch.with_active(|| ());
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl IPayloadHandler for Probe {
    fn handle(
        &self,
        _: PayloadBytes,
        ctx: &mut dyn IPayloadContext,
    ) -> Result<(), mod_api::PayloadHandleError> {
        assert!(matches!(
            ctx.server_host(),
            Err(PayloadContextError::WrongThread)
        ));
        let transport = self.transport.lock().unwrap().upgrade().unwrap();
        for _ in 0..2 {
            let before = self.events.stats().reserved_bytes;
            let result = ctx.enqueue_work(Arc::new(ReentrantWork {
                epoch: transport.epoch().unwrap(),
                drops: self.drops.clone(),
            }));
            if self.mode == Mode::Pressure {
                assert_eq!(result, Err(PayloadContextError::Backpressure));
                assert_eq!(self.events.stats().reserved_bytes, before);
            } else {
                assert_eq!(result, Ok(PayloadWorkStatus::Queued));
                assert!(
                    self.events.stats().reserved_bytes > before,
                    "Queued must already own main-provider credit"
                );
            }
            assert_eq!(
                self.events.stats().queued_items,
                0,
                "no publication during callback"
            );
        }
        if self.mode != Mode::Pressure {
            ctx.reply(&PayloadId(CHANNEL.into()), PayloadBytes(vec![1]))
                .unwrap();
        }
        // Also prove that no assertion was caught as a handler panic.
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.mode {
            Mode::Error => {
                return Err(mod_api::PayloadHandleError::Failed(
                    "deliberate handler error".into(),
                ));
            }
            Mode::Panic => panic!("deliberate handler panic after staging"),
            Mode::Cancel => transport.close(),
            Mode::Reconfigure => transport.begin_configuration(),
            Mode::PublicationClosed => drop(self.receiver.lock().unwrap().take()),
            _ => {}
        }
        Ok(())
    }
}
struct Registration(Arc<Probe>);
#[mod_api::__subscriptions]
impl Hooks for Registration {
    fn register(&self, registrar: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        registrar
            .payload_handlers()
            .registrar("1")
            .executes_on(HandlerThread::Network)
            .play_bidirectional(
                PayloadId(CHANNEL.into()),
                self.0.clone(),
                Some(self.0.clone()),
            );
    }
}
fn with_case(
    mode: Mode,
    bytes: usize,
    run: impl FnOnce(Arc<Probe>, Incoming, Endpoint, Box<dyn Fn() -> Vec<Vec<u8>>>),
) {
    let (events, receiver) = if mode == Mode::Pressure {
        crate::lan::event_channel_with_capacity(
            sunrise_mod_runtime::byte_channel::message_overhead::<NetEvent>(),
        )
    } else {
        crate::lan::event_channel()
    };
    let probe = Arc::new(Probe {
        mode,
        events,
        receiver: Mutex::new(Some(receiver)),
        transport: Mutex::new(Weak::new()),
        drops: Arc::default(),
        calls: AtomicUsize::new(0),
    });
    mod_platform::testing::with_mods(
        vec![ModEntry::new(
            "executor_probe",
            Box::new(Registration(probe.clone())),
        )]
        .into(),
        || {
            mod_platform::__private::part_tests::install_parts_fixture(mod_api::__private::mods())
                .unwrap();
            let transport = Transport::capture(Side::Server).unwrap();
            assert!(transport.local.owns(CHANNEL));
            transport
                .accept(transport.local.manifest.entries())
                .unwrap();
            transport.finish_configuration().unwrap();
            *probe.transport.lock().unwrap() = Arc::downgrade(&transport);
            let (sender, out) = PlayerSender::payload_executor_test_sender(transport.clone());
            let endpoint = Endpoint::server(sender);
            let incoming = Incoming::new(
                &transport,
                negotiation::Phase::Play,
                sunrise_protocol::packets::custom_payload::CustomPayload {
                    channel: CHANNEL.into(),
                    data: vec![0; bytes],
                },
            )
            .unwrap();
            run(probe, incoming, endpoint, Box::new(out));
        },
    );
}
fn queue(probe: &Probe, endpoint: &Endpoint) -> MainQueue {
    MainQueue::Server {
        events: probe.events.clone(),
        endpoint: endpoint.clone(),
        conn: Some(1),
    }
}

fn reply_count(snapshot: &dyn Fn() -> Vec<Vec<u8>>) -> usize {
    let reply = encode(
        negotiation::Phase::Play,
        Side::Server,
        &PayloadId(CHANNEL.into()),
        PayloadBytes(vec![1]),
    );
    snapshot().iter().filter(|body| *body == &reply).count()
}

#[test]
fn queued_work_reserves_before_acceptance_and_keeps_original_credit_until_last_drop() {
    with_case(Mode::Success, 3 << 20, |probe, incoming, endpoint, out| {
        let transport = endpoint.transport().clone();
        let original = Arc::downgrade(&incoming.lease);
        dispatch(incoming, endpoint.clone(), queue(&probe, &endpoint));
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
        assert_eq!(probe.events.stats().queued_items, 2);
        let receiver = probe.receiver.lock().unwrap().take().unwrap();
        let mut work: Vec<_> = receiver
            .take_turn()
            .into_iter()
            .map(|receipt| receipt.into_parts())
            .collect();
        assert_eq!(work.len(), 2);
        assert!(work.iter().all(|(event, _credit)| matches!(event, NetEvent::RawPayload(delivery) if matches!(&delivery.incoming.body, Body::Work(_)))));
        assert!(original.upgrade().is_some());
        assert!(transport.reserve_ingress(14 << 20).is_err());
        drop(work.pop());
        assert_eq!(probe.drops.load(Ordering::SeqCst), 1);
        assert!(original.upgrade().is_some());
        assert!(transport.reserve_ingress(14 << 20).is_err());
        drop(work);
        assert_eq!(probe.drops.load(Ordering::SeqCst), 2);
        assert!(original.upgrade().is_none());
        assert_eq!(probe.events.stats().reserved_bytes, 0);
        drop(
            transport
                .reserve_ingress(14 << 20)
                .expect("last work reference refunds physical credit"),
        );
        assert_eq!(reply_count(&*out), 1);
    });
}

#[test]
fn provider_pressure_refuses_work_before_acceptance() {
    with_case(Mode::Pressure, 1, |probe, incoming, endpoint, out| {
        let original = Arc::downgrade(&incoming.lease);
        dispatch(incoming, endpoint.clone(), queue(&probe, &endpoint));
        assert!(original.upgrade().is_none());
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
        assert_eq!(probe.drops.load(Ordering::SeqCst), 2);
        assert_eq!(probe.events.stats().reserved_bytes, 0);
        assert_eq!(probe.events.stats().queued_items, 0);
        assert_eq!(reply_count(&*out), 0);
    });
}

#[test]
fn handler_error_or_epoch_cancellation_publishes_no_staged_work_or_replies() {
    for mode in [Mode::Error, Mode::Panic, Mode::Cancel, Mode::Reconfigure] {
        with_case(mode, 1, |probe, incoming, endpoint, out| {
            let original = Arc::downgrade(&incoming.lease);
            dispatch(incoming, endpoint.clone(), queue(&probe, &endpoint));
            assert!(original.upgrade().is_none());
            assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
            assert_eq!(probe.drops.load(Ordering::SeqCst), 2);
            assert_eq!(probe.events.stats().reserved_bytes, 0);
            assert_eq!(probe.events.stats().queued_items, 0);
            assert_eq!(reply_count(&*out), 0);
            if matches!(mode, Mode::Error | Mode::Panic) {
                // The player is told which mod's handler failed, never its reason.
                let sent = out();
                let has = |text: &[u8]| {
                    sent.iter()
                        .any(|packet| packet.windows(text.len()).any(|w| w == text))
                };
                assert!(has(b"a mod's network handler failed (executor_probe)"));
                assert!(!has(b"deliberate handler error"));
            }
        });
    }
}

#[test]
fn main_root_blocks_later_network_delivery_and_keeps_its_ingress_charge() {
    mod_platform::__private::initialize().unwrap();
    for cancel in [None, Some(Mode::Reconfigure), Some(Mode::Cancel)] {
        with_case(
            Mode::Success,
            3 << 20,
            |probe, mut incoming, endpoint, out| {
                let transport = endpoint.transport();
                let tail = Incoming::new(
                    transport,
                    negotiation::Phase::Play,
                    sunrise_protocol::packets::custom_payload::CustomPayload {
                        channel: CHANNEL.into(),
                        data: vec![2],
                    },
                )
                .unwrap();
                let head_lease = Arc::downgrade(&incoming.lease);
                let tail_lease = Arc::downgrade(&tail.lease);
                incoming.receiver.thread = HandlerThread::Main;
                dispatch(incoming, endpoint.clone(), queue(&probe, &endpoint));
                dispatch(tail, endpoint.clone(), queue(&probe, &endpoint));
                let main = queue(&probe, &endpoint);
                dispatch_pending(&endpoint, &main);
                assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
                assert_eq!(reply_count(&*out), 0);
                assert_eq!(probe.events.stats().queued_items, 1);
                assert!(matches!(
                    transport.reserve_ingress(14 << 20),
                    Err(PayloadSendError::Backpressure)
                ));
                assert!(tail_lease.upgrade().is_some());
                match cancel {
                    Some(Mode::Reconfigure) => transport.begin_configuration(),
                    Some(Mode::Cancel) => transport.close(),
                    _ => {}
                }
                if cancel.is_some() {
                    assert!(tail_lease.upgrade().is_none());
                    assert!(
                        head_lease.upgrade().is_some(),
                        "the main provider still owns the head"
                    );
                }
                // Game consumes the root from the provider and returns its turn at completion.
                let receiver = probe.receiver.lock().unwrap().take().unwrap();
                drop(receiver.take_turn());
                assert!(head_lease.upgrade().is_none());
                dispatch_pending(&endpoint, &main);
                assert_eq!(
                    probe.calls.load(Ordering::SeqCst),
                    usize::from(cancel.is_none())
                );
                assert_eq!(reply_count(&*out), usize::from(cancel.is_none()));
                assert!(!transport.connection.as_ref().unwrap().payloads_pending());
                drop(receiver.take_turn());
                assert!(tail_lease.upgrade().is_none());
                assert_eq!(probe.events.stats().reserved_bytes, 0);
            },
        );
    }
}

#[test]
fn failed_publication_and_remaining_work_drop_outside_epoch_gate() {
    with_case(
        Mode::PublicationClosed,
        1,
        |probe, incoming, endpoint, out| {
            let original = Arc::downgrade(&incoming.lease);
            let main = queue(&probe, &endpoint);
            let (done, completion) = mpsc::channel();
            let worker = mod_api::testing::inherit_mods(move |_: &()| {
                dispatch(incoming, endpoint, main);
                done.send(()).unwrap();
            });
            let thread = std::thread::spawn(move || worker(&()));
            // Never join a stuck mutant: the timeout must fail this test rather
            // than hang the harness on opaque Work::drop reentering the gate.
            completion
                .recv_timeout(Duration::from_secs(5))
                .expect("dispatch deadlocked dropping opaque work under publication gate");
            thread.join().unwrap();
            assert!(original.upgrade().is_none());
            assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
            assert_eq!(
                probe.drops.load(Ordering::SeqCst),
                2,
                "both failed publication and unvisited staged work must be dropped"
            );
            assert_eq!(probe.events.stats().reserved_bytes, 0);
            assert_eq!(probe.events.stats().queued_items, 0);
            assert_eq!(reply_count(&*out), 0);
        },
    );
}

#[test]
fn disabled_owner_refuses_before_entering_its_native_handler() {
    with_case(Mode::Success, 1, |probe, incoming, endpoint, out| {
        let original = Arc::downgrade(&incoming.lease);
        assert!(mod_api::__private::set_mod_enabled("executor_probe", false));
        dispatch(incoming, endpoint.clone(), queue(&probe, &endpoint));
        assert!(endpoint.transport().closed());
        assert!(original.upgrade().is_none());
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        assert_eq!(probe.drops.load(Ordering::SeqCst), 0);
        assert_eq!(probe.events.stats().reserved_bytes, 0);
        assert_eq!(reply_count(&*out), 0);
    });
}

#[test]
fn a_network_handler_panic_fails_only_its_connection_and_disposes_staged_work() {
    mod_platform::__private::initialize().unwrap();
    struct AttributedWork {
        panic_on_drop: bool,
        drops: Arc<AtomicUsize>,
        drop_calls: Arc<Mutex<Vec<Option<mod_api::content::crash::ModCall>>>>,
    }
    impl PayloadWork for AttributedWork {
        fn run(&self, _: &mut dyn IPayloadContext) -> Result<(), String> {
            Ok(())
        }
    }
    impl Drop for AttributedWork {
        fn drop(&mut self) {
            self.drop_calls
                .lock()
                .unwrap()
                .push(mod_api::content::crash::current_native_call());
            self.drops.fetch_add(1, Ordering::SeqCst);
            if self.panic_on_drop {
                panic!("staged work drop panicked");
            }
        }
    }
    struct PerConnectionHandler {
        calls: AtomicUsize,
        drops: Arc<AtomicUsize>,
        drop_calls: Arc<Mutex<Vec<Option<mod_api::content::crash::ModCall>>>>,
    }
    impl IPayloadHandler for PerConnectionHandler {
        fn handle(
            &self,
            payload: PayloadBytes,
            ctx: &mut dyn IPayloadContext,
        ) -> Result<(), mod_api::PayloadHandleError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let fail = payload.0 == [13];
            assert_eq!(
                ctx.enqueue_work(Arc::new(AttributedWork {
                    panic_on_drop: fail,
                    drops: self.drops.clone(),
                    drop_calls: self.drop_calls.clone(),
                })),
                Ok(PayloadWorkStatus::Queued)
            );
            ctx.reply(&PayloadId(CHANNEL.into()), PayloadBytes(vec![1]))
                .unwrap();
            if fail {
                panic!("connection-local handler panic");
            }
            Ok(())
        }
    }
    struct Reg(Arc<PerConnectionHandler>);
    #[mod_api::__subscriptions]
    impl Hooks for Reg {
        fn register(&self, registrar: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
            registrar
                .payload_handlers()
                .registrar("1")
                .executes_on(HandlerThread::Network)
                .play_bidirectional(
                    PayloadId(CHANNEL.into()),
                    self.0.clone(),
                    Some(self.0.clone()),
                );
        }
    }

    let handler = Arc::new(PerConnectionHandler {
        calls: AtomicUsize::new(0),
        drops: Arc::default(),
        drop_calls: Arc::default(),
    });
    mod_platform::testing::with_mods(
        vec![ModEntry::new(
            "executor_probe",
            Box::new(Reg(handler.clone())),
        )]
        .into(),
        || {
            mod_platform::__private::part_tests::install_parts_fixture(mod_api::__private::mods())
                .unwrap();
            let local = Local::capture(Side::Server).unwrap();
            let connect = |conn_id: u32| {
                let transport = Transport::new(local.clone(), Side::Server);
                transport
                    .accept(transport.local.manifest.entries())
                    .unwrap();
                transport.finish_configuration().unwrap();
                let (sender, out) = PlayerSender::payload_executor_test_sender(transport.clone());
                let endpoint = Endpoint::server(sender);
                (transport, endpoint, out, conn_id)
            };
            let (events, receiver) = crate::lan::event_channel();
            let (first, first_endpoint, first_out, first_id) = connect(1);
            let (second, second_endpoint, second_out, second_id) = connect(2);
            let incoming = |transport: &Arc<Transport>, byte: u8| {
                Incoming::new(
                    transport,
                    negotiation::Phase::Play,
                    sunrise_protocol::packets::custom_payload::CustomPayload {
                        channel: CHANNEL.into(),
                        data: vec![byte],
                    },
                )
                .unwrap()
            };

            let bad = incoming(&first, 13);
            let bad_lease = Arc::downgrade(&bad.lease);
            dispatch(
                bad,
                first_endpoint.clone(),
                MainQueue::Server {
                    events: events.clone(),
                    endpoint: first_endpoint.clone(),
                    conn: Some(first_id),
                },
            );
            assert!(first.closed(), "panicking connection must disconnect");
            assert!(bad_lease.upgrade().is_none());
            assert_eq!(events.stats().queued_items, 0);
            assert_eq!(events.stats().reserved_bytes, 0);
            assert_eq!(reply_count(&first_out), 0);
            assert!(first_out().iter().any(|pkt| {
                pkt.windows(b"executor_probe".len())
                    .any(|w| w == b"executor_probe")
            }));
            assert_eq!(handler.drops.load(Ordering::SeqCst), 1);
            assert_eq!(
                handler.drop_calls.lock().unwrap()[0],
                Some(mod_api::content::crash::ModCall {
                    id: "executor_probe",
                    hook: "payload work drop",
                    part: None,
                })
            );

            // Second connection on the same Local still invokes the same handler cleanly.
            let good = incoming(&second, 7);
            let good_lease = Arc::downgrade(&good.lease);
            dispatch(
                good,
                second_endpoint.clone(),
                MainQueue::Server {
                    events: events.clone(),
                    endpoint: second_endpoint.clone(),
                    conn: Some(second_id),
                },
            );
            assert!(!second.closed(), "other connection must stay open");
            assert_eq!(handler.calls.load(Ordering::SeqCst), 2);
            assert_eq!(reply_count(&second_out), 1);
            assert_eq!(events.stats().queued_items, 1);
            drop(receiver.take_turn());
            assert_eq!(handler.drops.load(Ordering::SeqCst), 2);
            assert!(good_lease.upgrade().is_none());
            assert_eq!(events.stats().reserved_bytes, 0);
        },
    );
}

#[test]
fn a_panicking_work_destructor_on_refused_enqueue_fails_only_that_connection() {
    mod_platform::__private::initialize().unwrap();
    struct PanickingWork {
        panic_on_drop: bool,
        drop_calls: Arc<Mutex<Vec<Option<mod_api::content::crash::ModCall>>>>,
    }
    impl PayloadWork for PanickingWork {
        fn run(&self, _: &mut dyn IPayloadContext) -> Result<(), String> {
            Ok(())
        }
    }
    impl Drop for PanickingWork {
        fn drop(&mut self) {
            self.drop_calls
                .lock()
                .unwrap()
                .push(mod_api::content::crash::current_native_call());
            if self.panic_on_drop {
                panic!("refused work drop panicked");
            }
        }
    }
    struct IgnoresRefusal {
        drop_calls: Arc<Mutex<Vec<Option<mod_api::content::crash::ModCall>>>>,
    }
    impl IPayloadHandler for IgnoresRefusal {
        fn handle(
            &self,
            payload: PayloadBytes,
            ctx: &mut dyn IPayloadContext,
        ) -> Result<(), mod_api::PayloadHandleError> {
            let panic_on_drop = payload.0 == [13];
            let result = ctx.enqueue_work(Arc::new(PanickingWork {
                panic_on_drop,
                drop_calls: self.drop_calls.clone(),
            }));
            if panic_on_drop {
                assert_eq!(result, Err(PayloadContextError::Backpressure));
            } else {
                assert_eq!(result, Ok(PayloadWorkStatus::Queued));
            }
            // Even if the handler ignores the backpressure error and stages a reply,
            // a panicking work destructor must still fail this connection and publish nothing.
            ctx.reply(&PayloadId(CHANNEL.into()), PayloadBytes(vec![1]))
                .unwrap();
            Ok(())
        }
    }
    struct Reg(Arc<IgnoresRefusal>);
    #[mod_api::__subscriptions]
    impl Hooks for Reg {
        fn register(&self, registrar: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
            registrar
                .payload_handlers()
                .registrar("1")
                .executes_on(HandlerThread::Network)
                .play_bidirectional(
                    PayloadId(CHANNEL.into()),
                    self.0.clone(),
                    Some(self.0.clone()),
                );
        }
    }

    let drop_calls = Arc::new(Mutex::new(Vec::new()));
    let handler = Arc::new(IgnoresRefusal {
        drop_calls: drop_calls.clone(),
    });
    mod_platform::testing::with_mods(
        vec![ModEntry::new("executor_probe", Box::new(Reg(handler)))].into(),
        || {
            mod_platform::__private::part_tests::install_parts_fixture(mod_api::__private::mods())
                .unwrap();
            let local = Local::capture(Side::Server).unwrap();
            let connect = || {
                let transport = Transport::new(local.clone(), Side::Server);
                transport
                    .accept(transport.local.manifest.entries())
                    .unwrap();
                transport.finish_configuration().unwrap();
                let (sender, out) = PlayerSender::payload_executor_test_sender(transport.clone());
                let endpoint = Endpoint::server(sender);
                (transport, endpoint, out)
            };

            let (pressure_events, _pressure_rx) = crate::lan::event_channel_with_capacity(
                sunrise_mod_runtime::byte_channel::message_overhead::<NetEvent>(),
            );
            let (first, first_endpoint, first_out) = connect();
            let bad = Incoming::new(
                &first,
                negotiation::Phase::Play,
                sunrise_protocol::packets::custom_payload::CustomPayload {
                    channel: CHANNEL.into(),
                    data: vec![13],
                },
            )
            .unwrap();
            dispatch(
                bad,
                first_endpoint.clone(),
                MainQueue::Server {
                    events: pressure_events.clone(),
                    endpoint: first_endpoint,
                    conn: Some(1),
                },
            );
            assert!(
                first.closed(),
                "panicking work destructor must fail connection"
            );
            assert_eq!(reply_count(&first_out), 0);
            assert_eq!(pressure_events.stats().reserved_bytes, 0);
            assert_eq!(
                drop_calls.lock().unwrap()[0],
                Some(mod_api::content::crash::ModCall {
                    id: "executor_probe",
                    hook: "payload work drop",
                    part: None,
                })
            );

            // Second connection on the same Local with a healthy queue still succeeds.
            let (ok_events, ok_rx) = crate::lan::event_channel();
            let (second, second_endpoint, second_out) = connect();
            let good = Incoming::new(
                &second,
                negotiation::Phase::Play,
                sunrise_protocol::packets::custom_payload::CustomPayload {
                    channel: CHANNEL.into(),
                    data: vec![7],
                },
            )
            .unwrap();
            dispatch(
                good,
                second_endpoint.clone(),
                MainQueue::Server {
                    events: ok_events.clone(),
                    endpoint: second_endpoint,
                    conn: Some(2),
                },
            );
            assert!(!second.closed());
            assert_eq!(reply_count(&second_out), 1);
            assert_eq!(ok_events.stats().queued_items, 1);
            drop(ok_rx.take_turn());
        },
    );
}
