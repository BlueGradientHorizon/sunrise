use super::*;
use crate::{
    IPayloadContext, PayloadAccepted, PayloadBytes, PayloadContextError, PayloadHandleError,
    PayloadSendError, PayloadToken, PayloadWork, PayloadWorkStatus, PlayerRef, Text, Wire,
    payload_schema, typed_payload_version,
};
use std::sync::Mutex;

fn frozen(registrar: &PartRegistrar) -> mod_vocab::__private::game::Payloads {
    registrar
        .parts
        .payloads
        .as_ref()
        .unwrap()
        .as_ref()
        .clone()
        .into()
}

#[derive(Debug, PartialEq, Wire)]
struct Counter {
    counter: u8,
}

#[derive(Debug, PartialEq, Wire)]
struct Other {
    counter: i32,
}

fn ignore<T>(_: T, _: &mut dyn IPayloadContext) -> Result<(), String> {
    Ok(())
}

fn with_token(token: &PayloadToken, change: impl FnOnce(&mut PayloadToken)) -> PayloadToken {
    let mut token = token.clone();
    change(&mut token);
    token
}

#[test]
fn a_token_is_accepted_only_as_the_registrar_handed_it_out() {
    let mut registrar = PartRegistrar::new("test");
    let (up, (both_up, both_down)) = {
        let r = &mut registrar as &mut dyn Registrar;
        let mut payloads = r.payload_handlers();
        let mut view = payloads.registrar("1");
        let up = view.play_to_server_typed(PayloadId("test:up".into()), ignore::<Counter>);
        let both = view.common_bidirectional_typed(
            PayloadId("test:both".into()),
            ignore::<Counter>,
            ignore::<Counter>,
        );
        (up, both)
    };
    assert!(registrar.refused.is_empty(), "{:?}", registrar.refused);
    let table = frozen(&registrar);
    let up_declaration = table
        .declarations()
        .find(|declaration| declaration.id.0 == "test:up")
        .unwrap();
    assert_eq!(
        up_declaration.version,
        typed_payload_version("1", payload_schema::<Counter>())
    );
    for token in [up.token(), both_up.token(), both_down.token()] {
        assert_eq!(table.check_token(token), Ok(()));
    }
    let stale = |token: PayloadToken| {
        assert_eq!(
            table.check_token(&token),
            Err(PayloadSendError::StaleToken(token.id.clone())),
            "{token:?}"
        );
    };
    // Forged: another declaration's slot, a slot nobody has, another load.
    stale(with_token(up.token(), |t| t.slot = both_up.token().slot));
    stale(with_token(up.token(), |t| t.slot = 7));
    stale(with_token(up.token(), |t| t.load += 1));
    // Another value type, another direction, another protocol.
    stale(with_token(up.token(), |t| {
        t.schema = payload_schema::<Other>()
    }));
    stale(with_token(up.token(), |t| {
        t.direction = PayloadDirection::ToClient
    }));
    stale(with_token(up.token(), |t| {
        t.direction = PayloadDirection::Bidirectional
    }));
    stale(with_token(both_up.token(), |t| {
        t.protocol = PayloadProtocol::Play
    }));

    // The same registration in a later load hands out a token of its own.
    let mut later = PartRegistrar::new("test");
    let again = (&mut later as &mut dyn Registrar)
        .payload_handlers()
        .registrar("1")
        .play_to_server_typed(PayloadId("test:up".into()), ignore::<Counter>);
    assert_eq!(frozen(&later).check_token(again.token()), Ok(()));
    stale(again.token().clone());
}

#[test]
fn a_second_owner_of_an_id_is_refused_naming_the_first() {
    let mut registrar = PartRegistrar::new("test");
    let r = &mut registrar as &mut dyn Registrar;
    r.payload_handlers()
        .registrar("1")
        .play_to_server_typed(PayloadId("test:up".into()), ignore::<Counter>);
    let refused = r
        .payload_handlers()
        .registrar("2")
        .play_to_server_typed(PayloadId("test:up".into()), ignore::<Other>);
    assert_eq!(registrar.refused.len(), 1);
    let refusal = &registrar.refused[0];
    for name in ["test:up", "already registered by test"] {
        assert!(refusal.contains(name), "{refusal}");
    }
    // The refused registration's token is for no load, so it never sends.
    assert_eq!(refused.token().load, 0);

    // A raw version cannot pose as a typed one.
    (&mut registrar as &mut dyn Registrar)
        .payload_handlers()
        .registrar("1+payload-schema.0000000000000001")
        .play_to_server(PayloadId("test:raw".into()), Arc::new(Ignore));
    assert_eq!(registrar.refused.len(), 2);
    assert!(registrar.refused[1].contains("typed schema"));
}

struct Ignore;
impl IPayloadHandler for Ignore {
    fn handle(
        &self,
        _: PayloadBytes,
        _: &mut dyn IPayloadContext,
    ) -> Result<(), PayloadHandleError> {
        Ok(())
    }
}

/// Decoding runs before any context capability: a stand-in whose capabilities are never reached.
struct Unreached;
impl IPayloadContext for Unreached {
    fn player(&self) -> Result<PlayerRef, PayloadContextError> {
        unreachable!()
    }
    fn flow(&self) -> PayloadDirection {
        unreachable!()
    }
    fn protocol(&self) -> PayloadProtocol {
        unreachable!()
    }
    fn server_host(&mut self) -> Result<&mut dyn crate::ServerHost, PayloadContextError> {
        unreachable!()
    }
    fn client_host(&mut self) -> Result<&mut dyn crate::ClientHost, PayloadContextError> {
        unreachable!()
    }
    fn reply(
        &mut self,
        _: &PayloadId,
        _: PayloadBytes,
    ) -> Result<PayloadAccepted, PayloadSendError> {
        unreachable!()
    }
    fn check_payload_token(&self, _: &PayloadToken) -> Result<(), PayloadSendError> {
        unreachable!()
    }
    fn send_to(
        &mut self,
        _: &[PlayerRef],
        _: &PayloadId,
        _: PayloadBytes,
    ) -> Result<u32, PayloadSendError> {
        unreachable!()
    }
    fn finish_configuration_task(
        &mut self,
        _: crate::ConfigurationTaskRef,
    ) -> Result<(), crate::ConfigurationTaskError> {
        Err(crate::ConfigurationTaskError::WrongContext)
    }
    fn disconnect(&mut self, _: Text) {
        unreachable!()
    }
    fn enqueue_work(
        &mut self,
        _: Arc<dyn PayloadWork>,
    ) -> Result<PayloadWorkStatus, PayloadContextError> {
        unreachable!()
    }
}

#[derive(Debug, PartialEq, Wire)]
struct Labelled {
    counter: i32,
    labels: Vec<String>,
    chain: Chain,
}

#[derive(Debug, PartialEq, Wire)]
struct Chain(Option<Box<Chain>>);

#[test]
fn valid_and_malformed_wire_bodies_are_handled_in_both_directions() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let clients = seen.clone();
    let servers = seen.clone();
    let mut registrar = PartRegistrar::new("test");
    (&mut registrar as &mut dyn Registrar)
        .payload_handlers()
        .registrar("1")
        .play_bidirectional_typed(
            PayloadId("test:labelled".into()),
            move |value: Labelled, _| {
                servers
                    .lock()
                    .unwrap()
                    .push((PayloadDirection::ToServer, value));
                Ok(())
            },
            move |value: Labelled, _| {
                clients
                    .lock()
                    .unwrap()
                    .push((PayloadDirection::ToClient, value));
                Ok(())
            },
        );
    let table = frozen(&registrar);
    // Little-endian i32 7, one length-prefixed UTF-8 label, no recursive child.
    let valid = [7, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, b'a', 0];
    let nested = |levels: usize| {
        let mut body = vec![7, 0, 0, 0, 0, 0, 0, 0];
        body.extend(std::iter::repeat_n(1, levels));
        body.push(0);
        body
    };
    for direction in [PayloadDirection::ToServer, PayloadDirection::ToClient] {
        let receiver = table
            .handler("test:labelled", PayloadProtocol::Play, direction)
            .unwrap();
        for body in [
            // 4097 labels, one past the list bound.
            vec![7, 0, 0, 0, 1, 0x10, 0, 0],
            // Each next link adds an option and a box to the depth.
            nested(33),
            [valid.as_slice(), &[0xaa]].concat(),
            valid[..5].to_vec(),
        ] {
            let PayloadHandleError::Failed(refused) = receiver
                .handle(PayloadBytes(body), &mut Unreached)
                .unwrap_err();
            assert!(refused.contains("test:labelled"), "{refused}");
        }
        assert!(seen.lock().unwrap().is_empty());
        receiver
            .handle(PayloadBytes(valid.to_vec()), &mut Unreached)
            .unwrap();
        let expected = Labelled {
            counter: 7,
            labels: vec!["a".into()],
            chain: Chain(None),
        };
        assert_eq!(&*seen.lock().unwrap(), &[(direction, expected)]);
        seen.lock().unwrap().clear();
    }
}

#[test]
fn ordinary_only_wire_forms_refuse_registration_before_a_channel_exists() {
    fn refused<T: crate::wire::Wire + 'static>(
        view: &mut crate::PayloadRegistrar<'_>,
        id: &'static str,
    ) -> &'static str {
        let channel = view.play_to_server_typed::<T>(PayloadId(id.into()), ignore);
        let token = channel.token();
        assert_eq!(token.id.0, id);
        assert_eq!(token.load, 0, "{id}");
        id
    }

    let mut registrar = PartRegistrar::new("test");
    let mut payloads = (&mut registrar as &mut dyn Registrar).payload_handlers();
    let mut view = payloads.registrar("1");
    let ids = [
        refused::<crate::ModelPart>(&mut view, "test:model"),
        refused::<crate::Text>(&mut view, "test:text"),
        refused::<Option<&'static str>>(&mut view, "test:interned"),
        refused::<Option<serde_json::Value>>(&mut view, "test:json"),
        refused::<crate::LegacyRandomSource>(&mut view, "test:converted"),
        refused::<crate::LevelKey>(&mut view, "test:level_key"),
        refused::<crate::LevelDimension>(&mut view, "test:dimension"),
        refused::<Option<crate::Identifier>>(&mut view, "test:identifier"),
        refused::<crate::SpawnerKey>(&mut view, "test:spawner"),
        refused::<crate::CustomSpawners>(&mut view, "test:spawners"),
    ];
    drop(view);
    assert!(registrar.parts.payloads.is_none());
    assert_eq!(registrar.refused.len(), ids.len());
    for (refused, id) in registrar.refused.iter().zip(ids) {
        assert!(refused.contains(id), "{refused}");
    }
}

#[test]
fn a_table_made_by_a_raw_channel_is_still_a_load_of_its_own() {
    let register = || {
        let mut registrar = PartRegistrar::new("test");
        let r = &mut registrar as &mut dyn Registrar;
        let mut payloads = r.payload_handlers();
        let mut view = payloads.registrar("1");
        view.play_raw(PayloadId("test:raw".into()), None);
        let token = view.play_to_server_typed::<Counter>(PayloadId("test:up".into()), ignore);
        (registrar, token)
    };
    let (first, first_token) = register();
    let (second, second_token) = register();
    assert_ne!(first_token.token().load, 0);
    assert_eq!(frozen(&first).check_token(first_token.token()), Ok(()));
    assert_eq!(frozen(&second).check_token(second_token.token()), Ok(()));
    for (table, token) in [(&first, &second_token), (&second, &first_token)] {
        assert_eq!(
            frozen(table).check_token(token.token()),
            Err(PayloadSendError::StaleToken(token.token().id.clone()))
        );
    }
}

#[test]
fn a_typed_id_has_one_declaration_in_one_protocol() {
    let mut registrar = PartRegistrar::new("test");
    let r = &mut registrar as &mut dyn Registrar;
    let mut payloads = r.payload_handlers();
    let mut view = payloads.registrar("1");
    view.configuration_to_server_typed::<Other>(PayloadId("test:phased".into()), ignore);
    let play = view.play_to_server_typed::<Counter>(PayloadId("test:phased".into()), ignore);
    // A raw declaration in another protocol of a typed id is refused too.
    view.play_to_server(PayloadId("test:phased".into()), Arc::new(Ignore));
    assert_eq!(registrar.refused.len(), 2, "{:?}", registrar.refused);
    for refusal in &registrar.refused {
        assert!(refusal.contains("one declaration"), "{refusal}");
    }
    assert_eq!(play.token().load, 0);
}

#[test]
fn a_panicking_callback_fails_only_that_call_without_disabling_other_connections() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct PanickingRaw {
        calls: Arc<AtomicUsize>,
    }
    impl IPayloadHandler for PanickingRaw {
        fn handle(
            &self,
            payload: PayloadBytes,
            _: &mut dyn IPayloadContext,
        ) -> Result<(), PayloadHandleError> {
            assert!(mod_vocab::__private::game::retained_callback_active());
            self.calls.fetch_add(1, Ordering::SeqCst);
            if payload.0 == [13] {
                panic!("raw handler panics");
            }
            Ok(())
        }
    }

    let server_calls = Arc::new(AtomicUsize::new(0));
    let client_calls = Arc::new(AtomicUsize::new(0));
    let raw_calls = Arc::new(AtomicUsize::new(0));
    let mut registrar = PartRegistrar::new("payload_panic_isolated");
    {
        let r = &mut registrar as &mut dyn Registrar;
        let mut payloads = r.payload_handlers();
        let mut view = payloads.registrar("1");
        let server = server_calls.clone();
        let client = client_calls.clone();
        view.play_bidirectional_typed::<Counter>(
            PayloadId("test:panics_both".into()),
            move |value, _| {
                assert!(mod_vocab::__private::game::retained_callback_active());
                server.fetch_add(1, Ordering::SeqCst);
                if value.counter == 13 {
                    panic!("server handler panics");
                }
                Ok(())
            },
            move |value, _| {
                assert!(mod_vocab::__private::game::retained_callback_active());
                client.fetch_add(1, Ordering::SeqCst);
                if value.counter == 13 {
                    panic!("client handler panics");
                }
                Ok(())
            },
        );
        view.play_raw(
            PayloadId("test:panics_raw".into()),
            Some(Arc::new(PanickingRaw {
                calls: raw_calls.clone(),
            })),
        );
    }
    let table = frozen(&registrar);
    let check =
        |label: &str,
         calls: &AtomicUsize,
         resolve: &dyn Fn() -> mod_vocab::__private::game::PayloadReceiver| {
            let first = resolve();
            assert!(first.enabled(), "{label} receiver starts enabled");
            let PayloadHandleError::Failed(error) = first
                .handle(PayloadBytes(vec![13]), &mut Unreached)
                .unwrap_err();
            assert!(error.contains("panicked"), "{label}: {error}");
            assert!(
                !error.contains("disabled"),
                "{label} panic must not claim the callback is disabled: {error}"
            );
            // The handler stays enabled for both this receiver and any later connection's receiver.
            for receiver in [first, resolve()] {
                assert!(receiver.enabled(), "{label} must stay enabled after panic");
                assert_eq!(
                    receiver.handle(PayloadBytes(vec![7]), &mut Unreached),
                    Ok(()),
                    "{label} healthy delivery must still run after another call panicked"
                );
            }
            assert_eq!(calls.load(Ordering::SeqCst), 3, "{label} call count");
        };

    check("server", &server_calls, &|| {
        table
            .handler(
                "test:panics_both",
                PayloadProtocol::Play,
                PayloadDirection::ToServer,
            )
            .unwrap()
    });
    check("client", &client_calls, &|| {
        table
            .handler(
                "test:panics_both",
                PayloadProtocol::Play,
                PayloadDirection::ToClient,
            )
            .unwrap()
    });
    check("raw", &raw_calls, &|| {
        let mut receivers = table.raw_receivers("test:panics_raw");
        assert_eq!(receivers.len(), 1);
        receivers.pop().unwrap()
    });

    let notices = mod_vocab::__private::game::take_callback_panic_notices();
    assert!(
        !notices.iter().any(|n| n.mod_id == "payload_panic_isolated"),
        "payload panics must not queue a session-disabled notice: {notices:?}"
    );
}

#[test]
fn call_owned_payload_work_is_disposed_under_owner_attribution_without_global_poison() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct Work {
        panic_run: bool,
        panic_drop: bool,
        ran: Arc<AtomicUsize>,
        dropped: Arc<AtomicUsize>,
        retained_on_drop: Arc<AtomicBool>,
    }
    impl PayloadWork for Work {
        fn run(&self, _: &mut dyn IPayloadContext) -> Result<(), String> {
            assert!(mod_vocab::__private::game::retained_callback_active());
            self.ran.fetch_add(1, Ordering::SeqCst);
            if self.panic_run {
                panic!("work run panicked");
            }
            Ok(())
        }
    }
    impl Drop for Work {
        fn drop(&mut self) {
            self.retained_on_drop.store(
                mod_vocab::__private::game::retained_callback_active(),
                Ordering::SeqCst,
            );
            self.dropped.fetch_add(1, Ordering::SeqCst);
            if self.panic_drop {
                panic!("work drop panicked");
            }
        }
    }

    let mut registrar = PartRegistrar::new("payload_work_disposal");
    (&mut registrar as &mut dyn Registrar)
        .payload_handlers()
        .registrar("1")
        .play_to_server_typed::<Counter>(PayloadId("test:work".into()), ignore);
    let table = frozen(&registrar);
    let receiver = table
        .handler(
            "test:work",
            PayloadProtocol::Play,
            PayloadDirection::ToServer,
        )
        .unwrap();

    let make = |panic_run, panic_drop| {
        let ran = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));
        let retained_on_drop = Arc::new(AtomicBool::new(false));
        let work: Arc<dyn PayloadWork> = Arc::new(Work {
            panic_run,
            panic_drop,
            ran: ran.clone(),
            dropped: dropped.clone(),
            retained_on_drop: retained_on_drop.clone(),
        });
        (work, ran, dropped, retained_on_drop)
    };

    // 1. Work returns Ok, but its Drop panics: contained under owner attribution, no poison.
    let (work, ran, dropped, retained) = make(false, true);
    let err = receiver.run_work(work, &mut Unreached).unwrap_err();
    assert!(err.contains("work drop panicked"), "{err}");
    assert_eq!(ran.load(Ordering::SeqCst), 1);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(retained.load(Ordering::SeqCst));
    assert!(receiver.enabled());

    // 2. Both run and Drop panic: separate containment prevents double-panic abort.
    let (work, ran, dropped, retained) = make(true, true);
    let err = receiver.run_work(work, &mut Unreached).unwrap_err();
    assert!(err.contains("work run panicked"), "{err}");
    assert_eq!(ran.load(Ordering::SeqCst), 1);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(retained.load(Ordering::SeqCst));
    assert!(receiver.enabled());

    // 3. Cancelled/discarded work disposed without running: Drop still contained and attributed.
    let (work, ran, dropped, retained) = make(false, true);
    let err = receiver.dispose_work(work).unwrap_err();
    assert!(err.contains("work drop panicked"), "{err}");
    assert_eq!(ran.load(Ordering::SeqCst), 0);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(retained.load(Ordering::SeqCst));
    assert!(receiver.enabled());

    // 4. Subsequent healthy work on the same receiver still runs cleanly.
    let (work, ran, dropped, retained) = make(false, false);
    assert_eq!(receiver.run_work(work, &mut Unreached), Ok(()));
    assert_eq!(ran.load(Ordering::SeqCst), 1);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(retained.load(Ordering::SeqCst));

    let notices = mod_vocab::__private::game::take_callback_panic_notices();
    assert!(
        !notices.iter().any(|n| n.mod_id == "payload_work_disposal"),
        "payload work panics must not queue a session-disabled notice: {notices:?}"
    );
}
