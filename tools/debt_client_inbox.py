#!/usr/bin/env python3
"""Move the client ingress receiver out of Session, preserving tagged packet publication."""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
CLIENT = "crates/sunrise-client/src/"


def read(path):
    return (ROOT / path).read_text()


def write(path, source):
    (ROOT / path).write_text(source)


def replace(source, old, new, count=1):
    assert source.count(old) == count, old
    return source.replace(old, new)


path = CLIENT + "net.rs"
source = read(path)
source = replace(source, "pub mod ingress;", "mod client_inbox;\npub use client_inbox::{ClientInbox, EventSender, QueuedEvent};\npub mod ingress;")
source = replace(source, "fn event_channel() -> (ingress::Producer<Event>, ingress::Consumer<Event>) {\n    ingress::channel(96 << 20, Event::ingress_charge).expect(\"positive incoming byte budget\")\n}", "#[cfg(test)]\nfn event_channel() -> (EventSender, ingress::Consumer<QueuedEvent>) {\n    let inbox = ClientInbox::default();\n    (inbox.connection(0), inbox.into_receiver())\n}")
source = replace(source, "(Self, ingress::ReceiveLease<Self>, usize, usize)", "(QueuedEvent, ingress::ReceiveLease<QueuedEvent>, usize, usize)")
source = replace(source, "    events: ingress::Consumer<Event>,\n", "")
source = replace(source, "ingress::Producer<Event>", "EventSender", count=8)
source = replace(source, "let (_send, events) = event_channel();\n        Self {\n            payload_events: _send,\n            events,", "let serial = next_serial();\n        Self {\n            payload_events: inbox.connection(serial),")
source = replace(source, "pub fn replay(name: &str)", "pub fn replay(inbox: &ClientInbox, name: &str)")
start, end = source.index("    pub fn connect("), source.index("    pub fn poll(")
constructors = source[start:end]
constructors = re.sub(r"(    (?:pub )?fn (?:connect(?:_\w+)?|start)\(\n)", r"\1        inbox: &ClientInbox,\n", constructors)
constructors = replace(constructors, "            address, name, auth", "            inbox, address, name, auth")
constructors = replace(constructors, "Self::start(address, name, auth", "Self::start(inbox, address, name, auth")
constructors = replace(constructors, "Self::start(\n            address,", "Self::start(\n            inbox,\n            address,")
constructors = replace(constructors, 'Self::start(\n            "local",', 'Self::start(\n            inbox,\n            "local",')
constructors = replace(constructors, "let (send, events) = event_channel();", "let serial = next_serial();\n        let send = inbox.connection(serial);")
constructors = replace(constructors, "            serial: next_serial(),", "            serial,", count=2)
constructors = replace(constructors, "            events,\n            payload_events,", "            payload_events,")
source = source[:start] + constructors + source[end:]
source = replace(source, '    /// The frozen receiving turn, retaining every input credit through its actor.\n    #[sunrise_mod_api_macros::replaceable]\n    pub fn poll(&self) -> Vec<ingress::OwnedReceipt<Event>> {\n        self.events.take_turn()\n    }\n', '')
source = replace(source, "            let session = Session::connect(\n", "            let inbox = ClientInbox::default();\n            let session = Session::connect(\n                &inbox,\n")
source = replace(source, "            let session = connect(\n", "            let inbox = ClientInbox::default();\n            let _session = connect(\n                &inbox,\n")
source = replace(source, "                let (event, credit) = session\n                    .events\n", "                let (event, credit) = inbox.receiver_for_test()\n")
source = re.sub(r"let \(event, (\w+)\) = (events\.(?:try_recv_owned|recv_timeout_owned)\([^\n]+into_parts\(\);)", r"let (QueuedEvent { event, .. }, \1) = \2", source)
source = replace(source, "let (event, credit) = inbox.receiver_for_test()", "let (QueuedEvent { event, .. }, credit) = inbox.receiver_for_test()")
source = replace(source, "Some((Event::Transfer { host, port: 25566 }, _credit))", "Some((QueuedEvent { event: Event::Transfer { host, port: 25566 }, .. }, _credit))")
source = replace(source, "        let session = Session::replay(\"StudioTester\");", "        let session = Session::replay(&ClientInbox::default(), \"StudioTester\");")
source = replace(source, "let (_send, events) = event_channel();", "let (send, _events) = event_channel();")
source = replace(source, "let (_, events) = event_channel();", "let (send, _events) = event_channel();")
source = source.replace("let (send, events) = event_channel();\n        let session = Session {", "let (send, _events) = event_channel();\n        let session = Session {")
source = re.sub(r"(?m)^( +)events,\n\1payload_events: event_channel\(\).0,", r"\1payload_events: send.clone(),", source)
write(path, source)

for name, old, new in [
    ("raw_payloads/main_queue.rs", "Event,\n        ingress", "Event, EventSender, QueuedEvent,\n        ingress"),
    ("raw_payloads/endpoint.rs", "net::{Event, PreparedPayload,", "net::{EventSender, PreparedPayload,"),
]:
    path = CLIENT + name
    source = replace(read(path), old, new)
    if name.endswith("main_queue.rs"):
        source = replace(source, "Client(Producer<Event>)", "Client(EventSender)")
        source = replace(source, "Prepared<Event>", "Prepared<QueuedEvent>")
        source = replace(source, "Refused<Event>", "Refused<QueuedEvent>")
    else:
        source = replace(source, "crate::net::ingress::Producer<Event>", "EventSender", count=2)
    write(path, source)

path = CLIENT + "game.rs"
source = replace(read(path), "    remote: Option<remote::Remote>,", "    remote: Option<remote::Remote>,\n    client_inbox: sunrise_client::net::ClientInbox,")
source = replace(source, "            remote: None,\n            realm_session:", "            remote: None,\n            client_inbox: Default::default(),\n            realm_session:")
write(path, source)

path = CLIENT + "game/network/remote/mod.rs"
source = read(path)
for old, new in [
    ("Some(pipe) => Session::connect_local(\n", "Some(pipe) => Session::connect_local(\n                &self.client_inbox,\n"),
    ("Some(cookies) => Session::connect_transferred(\n", "Some(cookies) => Session::connect_transferred(\n                        &self.client_inbox,\n"),
    ("                        connect(\n", "                        connect(\n                            &self.client_inbox,\n"),
    ("Session::replay(name)", "Session::replay(&self.client_inbox, name)"),
]:
    source = replace(source, old, new)
write(path, source)

path = CLIENT + "game/network/remote/raw_ingress.rs"
source = read(path)
source = replace(source, "pub(super) fn receive_credited_backlog(&mut self, events: &mut VecDeque<Received>)", "fn receive_credited_backlog(&mut self, inbox: &sunrise_client::net::ClientInbox, events: &mut VecDeque<Received>)")
source = replace(source, "for receipt in self.session.poll() {\n            let (event, credit) = receipt.into_parts();", "for receipt in inbox.poll() {\n            let (queued, credit) = receipt.into_parts();\n            if queued.serial != self.session.serial() {\n                continue;\n            }\n            let event = queued.event;")
source = replace(source, "r.receive_credited_backlog(&mut events)", "r.receive_credited_backlog(&self.client_inbox, &mut events)")
write(path, source)

path = CLIENT + "game/network/remote/client_level_lifecycle_tests.rs"
source = replace(read(path), "    tx.send(Event::Reconfiguring).unwrap();\n    let (event, credit) = rx.try_recv_owned().unwrap().into_parts();", "    tx.send(sunrise_client::net::QueuedEvent {\n        serial: game.remote.as_ref().unwrap().session.serial(),\n        event: Event::Reconfiguring,\n    }).unwrap();\n    let (queued, credit) = rx.try_recv_owned().unwrap().into_parts();")
write(path, replace(source, "Received::Event(event, credit)", "Received::Event(queued.event, credit)"))

path = "crates/sunrise-client-gameplay/src/remote/chunks.rs"
source = replace(read(path), "net::Event,", "net::{Event, QueuedEvent},")
write(path, replace(source, "ReceiveLease<Event>", "ReceiveLease<QueuedEvent>", count=4))

path = CLIENT + "net/mod_packet_tests.rs"
write(path, replace(read(path), 'Session::replay("', 'Session::replay(&ClientInbox::default(), "', count=2))

path = "crates/sunrise-client/tests/datapack_join.rs"
source = replace(read(path), "net::{Event, Session}", "net::{ClientInbox, Event, Session}")
source = replace(source, "-> Session {\n    Session::connect(\n", "-> (Session, ClientInbox) {\n    let inbox = ClientInbox::default();\n    let session = Session::connect(\n        &inbox,\n")
source = replace(source, "        Arc::clone(data),\n    )", "        Arc::clone(data),\n    );\n    (session, inbox)")
source = replace(source, "session: &Session", "session: &(Session, ClientInbox)", count=2)
source = replace(source, "session.poll()", "session.1.poll()")
source = replace(source, "let (event, _credit) = receipt.into_parts();\n            if let Some(found) = done(host, event)", "let (queued, _credit) = receipt.into_parts();\n            if let Some(found) = done(host, queued.event)")
write(path, source)
