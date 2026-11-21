use super::*;
use crate::recipe_book::settings_codec::{decode, encode};
fn flags(open: bool, filtering: bool) -> BookFlags {
    BookFlags { open, filtering }
}
fn id(key: &str) -> ResourceId {
    ResourceId::try_from(key.to_owned()).unwrap()
}
fn registered() -> BTreeSet<ResourceId> {
    ["demo:pot", "demo:oven"].map(id).into()
}
#[test]
fn authoritative_screen_pair_reconnect_and_pending_intent() {
    let mut saved = CustomBooks::default();
    let mut server = Authority::negotiate(7, registered().into_iter().collect(), &registered());
    let mut client = ClientView::default();
    assert!(client.receive(server.snapshot(&saved), &registered()));
    assert_eq!(client.books.get("demo:pot"), flags(false, false));
    let now = Instant::now();
    let (reply, changed) =
        server.request(&mut saved, 7, 1, &id("demo:pot"), flags(true, false), now);
    assert!(changed);
    assert!(client.receive(reply, &registered()));
    let (reply, changed) =
        server.request(&mut saved, 7, 2, &id("demo:pot"), flags(true, true), now);
    assert!(changed);
    assert!(client.receive(reply, &registered()));
    assert_eq!(saved.get("demo:pot"), flags(true, true));
    assert_eq!(saved.get("demo:oven"), flags(false, false));
    // A pending request is not passed to the authority and never reaches saved state.
    let _pending = Message::Request {
        epoch: 7,
        number: 3,
        key: id("demo:pot"),
        flags: flags(false, true),
    };
    let reconnected = Authority::negotiate(8, registered().into_iter().collect(), &registered());
    let mut next = ClientView::default();
    assert!(next.receive(reconnected.snapshot(&saved), &registered()));
    assert_eq!(next.books.get("demo:pot"), flags(true, true));
    let (reply, changed) =
        server.request(&mut saved, 6, 99, &id("demo:pot"), flags(false, false), now);
    assert!(!changed);
    assert!(matches!(reply, Message::Snapshot { .. }));
    assert!(
        !server
            .request(&mut saved, 7, 2, &id("demo:pot"), flags(false, false), now)
            .1
    );
    assert!(
        !server
            .request(
                &mut saved,
                7,
                4,
                &id("unknown:book"),
                flags(true, true),
                now
            )
            .1
    );
    assert_eq!(saved.get("demo:pot"), flags(true, true));
}
#[test]
fn rejects_unknown_books_rate_excess_and_revision_gaps() {
    let mut books = CustomBooks::default();
    books.set(&id("absent:book"), flags(true, false));
    let mut server =
        Authority::negotiate(1, vec![id("demo:pot"), id("absent:book")], &registered());
    let snapshot = server.snapshot(&books);
    assert!(
        matches!(&snapshot,Message::Snapshot { entries,.. } if entries==&vec![(id("demo:pot"),flags(false,false))])
    );
    let mut client = ClientView::default();
    assert!(client.receive(snapshot, &registered()));
    let now = Instant::now();
    for number in 1..=20 {
        assert!(
            !server
                .request(
                    &mut books,
                    1,
                    number,
                    &id("demo:pot"),
                    flags(false, false),
                    now
                )
                .1
        );
    }
    assert!(
        !server
            .request(&mut books, 1, 21, &id("demo:pot"), flags(true, true), now)
            .1
    );
    assert_eq!(books.get("demo:pot"), flags(false, false));
    assert!(
        !server
            .request(
                &mut books,
                1,
                22,
                &id("demo:oven"),
                flags(true, true),
                now + Duration::from_secs(1)
            )
            .1
    );
    assert!(!client.receive(
        Message::Update {
            epoch: 1,
            revision: 2,
            key: id("demo:pot"),
            flags: flags(true, true)
        },
        &registered()
    ));
    assert!(!client.ready);
    assert_eq!(client.books.get("demo:pot"), flags(false, false));
    assert!(client.receive(server.snapshot(&books), &registered()));
}
#[test]
fn settings_codec_rejects_malformed_messages() {
    assert_eq!(
        encode(&Message::Offer(vec![id("demo:pot")])),
        Ok(b"\x00\x01\x08demo:pot".to_vec())
    );
    assert_eq!(decode(&[4]), Some(Message::Resnapshot));
    let messages = [
        Message::Hello,
        Message::Ready,
        Message::Offer(vec![id("demo:pot")]),
        Message::Snapshot {
            epoch: 4,
            revision: 0,
            entries: vec![(id("demo:pot"), flags(true, false))],
        },
        Message::Request {
            epoch: 4,
            number: 1,
            key: id("demo:pot"),
            flags: flags(true, true),
        },
        Message::Update {
            epoch: 4,
            revision: 1,
            key: id("demo:pot"),
            flags: flags(true, true),
        },
        Message::Resnapshot,
    ];
    for message in messages {
        let encoded = encode(&message).unwrap();
        assert_eq!(decode(&encoded), Some(message));
        for end in 0..encoded.len() {
            assert!(decode(&encoded[..end]).is_none());
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(decode(&trailing).is_none());
    }
    // Truncated, negative, overlong and impossible counts; then short and invalid IDs.
    for bytes in [
        &b"\x00\x80"[..],
        b"\x00\xff\xff\xff\xff\x0f",
        b"\x00\x80\x80\x80\x80\x80\x00",
        b"\x00\xff\xff\xff\xff\x07\x03a:b",
        b"\x00\x01\x04a:b",
        b"\x00\x01\x03A:b",
    ] {
        assert!(decode(bytes).is_none(), "{bytes:?}");
    }
    let duplicate = Message::Snapshot {
        epoch: 1,
        revision: 0,
        entries: vec![(id("demo:pot"), flags(false, false)); 2],
    };
    assert!(decode(&encode(&duplicate).unwrap()).is_none());
    let mut malformed = encode(&Message::Request {
        epoch: 1,
        number: 1,
        key: id("demo:pot"),
        flags: flags(false, false),
    })
    .unwrap();
    *malformed.last_mut().unwrap() = 2;
    assert!(decode(&malformed).is_none());
}

#[test]
fn large_settings_catalogue_round_trips_over_the_registered_channel() {
    use sunrise_protocol::{
        frame,
        packets::{configuration::CustomPayload, play},
    };

    // This exceeds the former book count, ID size, and aggregate byte ceilings,
    // while fitting a real play CustomPayload packet including its envelope.
    let prefix = format!("demo:{}", "cook/".repeat(100));
    let registered: BTreeSet<_> = (0..1_100).map(|i| id(&format!("{prefix}{i:04}"))).collect();
    let offered = Message::Offer(registered.iter().cloned().collect());
    let data = encode(&offered).unwrap();
    assert_eq!(&data[..3], &[0, 0xcc, 0x08]); // 1100 as a VarInt
    assert!(data.len() > 512 * 1024);
    let body = play::Serverbound::CustomPayload(CustomPayload {
        channel: CHANNEL.into(),
        data,
    })
    .try_encode()
    .unwrap();
    assert!(frame::encode(&body, None).is_ok());
    let mut reader = sunrise_protocol::buf::Reader::new(&body);
    let packet_id = reader.varint().unwrap();
    let play::Serverbound::CustomPayload(payload) =
        play::Serverbound::decode(packet_id, reader.rest()).unwrap()
    else {
        panic!("a registered custom payload");
    };
    assert_eq!(decode(&payload.data), Some(offered));

    let snapshot = Message::Snapshot {
        epoch: 3,
        revision: 7,
        entries: registered
            .iter()
            .map(|key| (key.clone(), flags(true, false)))
            .collect(),
    };
    let decoded = decode(&encode(&snapshot).unwrap()).unwrap();
    assert_eq!(decoded, snapshot);
    let mut client = ClientView::default();
    assert!(client.receive(decoded, &registered));
    assert!(client.ready);
    assert!(
        registered
            .iter()
            .all(|key| client.books.get(key.as_str()) == flags(true, false))
    );
}

#[test]
fn identifier_length_is_a_wire_constraint_only() {
    use sunrise_core::vanilla_bytes::Writer;

    let at_limit = format!("a:{}", "b".repeat(32_765));
    let over = format!("{at_limit}c");
    let offer = Message::Offer(vec![id(&at_limit), id(&over)]);
    assert_eq!(
        decode(&encode(&offer).unwrap()),
        Some(Message::Offer(vec![id(&at_limit)])),
        "an unencodable local key does not disable the other book"
    );
    assert!(
        encode(&Message::Request {
            epoch: 1,
            number: 1,
            key: id(&over),
            flags: flags(true, false),
        })
        .is_err()
    );
    let mut raw = Writer::new();
    raw.bytes(&[0, 1]);
    raw.string(&over);
    assert!(decode(&raw.data).is_none());
}

#[test]
fn payload_budget_bounds_all_snapshot_producing_routes() {
    let mut budget = RequestBudget::default();
    let now = Instant::now();
    for _ in 0..20 {
        assert!(budget.accept(now));
    }
    for _ in 0..100 {
        assert!(!budget.accept(now));
    }
    assert!(budget.accept(now + Duration::from_secs(1)));
    assert!(decode(&[5, 0]).is_none());
    assert!(decode(&[6, 1]).is_none());
}

#[test]
fn connected_replacement_removes_destination_only_flags() {
    let mut books = CustomBooks::default();
    books.set(&id("demo:oven"), flags(true, true));
    let mut server = Authority::negotiate(12, registered().into_iter().collect(), &registered());
    let mut client = ClientView::default();
    assert!(client.receive(server.snapshot(&books), &registered()));
    let mut imported = CustomBooks::default();
    imported.set(&id("demo:pot"), flags(true, false));
    books = imported;
    server.revision += 1;
    assert!(client.receive(server.snapshot(&books), &registered()));
    assert_eq!(client.books.get("demo:pot"), flags(true, false));
    assert_eq!(client.books.get("demo:oven"), flags(false, false));
    assert!(!client.receive(
        Message::Update {
            epoch: 12,
            revision: 1,
            key: id("demo:oven"),
            flags: flags(true, true)
        },
        &registered()
    ));
    assert_eq!(client.books.get("demo:oven"), flags(false, false));
}
