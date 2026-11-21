use super::*;
use sunrise_protocol::{
    buf::Reader,
    connection_groups::{self, ChunkEntry, Message, UpdateEntry},
    packets::play::Clientbound as C,
};

fn negotiated_transport() -> Arc<crate::raw_payloads::Transport> {
    let transport =
        crate::raw_payloads::Transport::capture(crate::raw_payloads::Side::Server).unwrap();
    transport
        .accept(transport.local.manifest.entries())
        .unwrap();
    transport.finish_configuration().unwrap();
    transport
}

fn decode_packets(rx: &std::sync::mpsc::Receiver<crate::lan::Out>) -> Vec<C> {
    let mut out = Vec::new();
    while let Ok(item) = rx.try_recv() {
        let bodies = match item {
            crate::lan::Out::Packet(body) => vec![body],
            crate::lan::Out::Packets(bodies) => bodies,
            crate::lan::Out::Disconnect(_) => panic!("unexpected disconnect"),
        };
        for body in bodies {
            let mut r = Reader::new(&body);
            let id = r.varint().unwrap();
            if let Ok(packet) = C::decode(id, r.rest()) {
                out.push(packet);
            }
        }
    }
    out
}

fn group_messages(packets: &[C]) -> Vec<Message> {
    packets
        .iter()
        .filter_map(|p| match p {
            C::CustomPayload(p) if p.channel == connection_groups::CHANNEL => {
                Some(connection_groups::decode(&p.data).unwrap())
            }
            _ => None,
        })
        .collect()
}

fn hub_with_sunrise_and_vanilla() -> (
    Hub,
    std::sync::mpsc::Receiver<crate::lan::Out>,
    std::sync::mpsc::Receiver<crate::lan::Out>,
) {
    let (mut hub, _old) = hub_with_a_guest();
    let (tx_sunrise, rx_sunrise) = std::sync::mpsc::channel();
    let sunrise_sender = crate::lan::PlayerSender::new_with_payload_transport(
        tx_sunrise,
        Arc::default(),
        negotiated_transport(),
    );
    hub.guests.update(0, |g| {
        g.sender = sunrise_sender;
        g.max_unacknowledged = 64;
    });
    let (tx_vanilla, rx_vanilla) = std::sync::mpsc::channel();
    let mut vanilla = Guest::new(
        crate::lan::Joined {
            conn: 2,
            uuid: 2,
            name: "Vanilla".into(),
            properties: Vec::new(),
            info: sunrise_protocol::packets::configuration::ClientInformation::new(8),
            brand: "vanilla".into(),
            sender: crate::lan::PlayerSender::new(tx_vanilla, Arc::default()),
            latency: Arc::new(std::sync::atomic::AtomicI32::new(0)),
            address: "127.0.0.1:25566".parse().unwrap(),
            expected: false,
        },
        2,
        [0.5, 64.0, 0.5],
        8,
    );
    vanilla.placed = true;
    vanilla.dimension = "minecraft:overworld".into();
    vanilla.max_unacknowledged = 64;
    hub.guests.push(vanilla);
    (hub, rx_sunrise, rx_vanilla)
}

#[test]
fn chunk_send_delivers_connection_groups_after_batch_only_to_negotiated_guests() {
    let (mut hub, rx_sunrise, rx_vanilla) = hub_with_sunrise_and_vanilla();
    hub.ticks = 10;
    let mut world = World::new(sunrise_world::registry::BlockStateId(0), 0);
    for cz in -1..=1 {
        for cx in -1..=1 {
            let mut chunk = Chunk::empty(cx, cz);
            chunk.lit = true;
            if (cx, cz) == (0, 0) {
                chunk.set(3, 64, 5, 1);
                chunk.set_connection_group(3, 64, 5, Some(77));
            }
            world.chunks.insert((cx, cz), Arc::new(chunk));
        }
    }

    let mut next_entity = 10;
    let border = crate::worldborder::WorldBorder::default();
    let ctx = TickCtx {
        sea_level: 63,
        world: &mut world,
        entities: &[],
        next_entity: &mut next_entity,
        spawn: [0.5, 64.0, 0.5],
        spawn_angle: [0.0; 2],
        spawn_dimension: "minecraft:overworld",
        difficulty: 2,
        difficulty_locked: false,
        clock_rate: 1.0,
        tick_rate: 20.0,
        tick_frozen: false,
        tick_steps: Default::default(),
        collision: None,
        border: &border,
        streaming: None,
    };
    hub.update_chunks(&ctx);

    let sunrise = decode_packets(&rx_sunrise);
    let batch_end = sunrise
        .iter()
        .position(|p| matches!(p, C::ChunkBatchFinished(_)))
        .expect("ChunkBatchFinished sent");
    let group_pkt = sunrise
        .iter()
        .position(|p| matches!(p, C::CustomPayload(cp) if cp.channel == connection_groups::CHANNEL))
        .expect("sunrise:connection_groups sent");
    assert!(batch_end < group_pkt, "group snapshot follows chunk batch");
    assert_eq!(
        group_messages(&sunrise),
        vec![Message::Chunk {
            cx: 0,
            cz: 0,
            replace: true,
            entries: vec![ChunkEntry {
                local_x: 3,
                y: 64,
                local_z: 5,
                group: 77,
            }],
        }]
    );

    let vanilla = decode_packets(&rx_vanilla);
    assert!(
        vanilla
            .iter()
            .any(|p| matches!(p, C::ChunkBatchFinished(_))),
        "vanilla guest still received chunk batch"
    );
    assert!(
        group_messages(&vanilla).is_empty(),
        "non-negotiated guest never receives sunrise:connection_groups"
    );
}

#[test]
fn incremental_updates_dedup_and_filter_by_negotiation_and_sent_chunks() {
    let (mut hub, rx_sunrise, rx_vanilla) = hub_with_sunrise_and_vanilla();
    let mut world = World::new(sunrise_world::registry::BlockStateId(0), 0);
    world.connection_group_log = Some(Vec::new());
    world.chunks.insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    world.chunks.insert((2, 0), Arc::new(Chunk::empty(2, 0)));
    hub.guests.update(0, |g| _ = g.sent_chunks.insert((0, 0)));
    hub.guests.update(1, |g| _ = g.sent_chunks.insert((0, 0)));

    // Multiple edits to the same cell collapse to the last value; an unsent chunk (2, 0) is
    // omitted from incremental updates.
    world
        .set_connection_group(1, 64, 2, Some(10), true)
        .unwrap();
    world
        .set_connection_group(1, 64, 2, Some(20), true)
        .unwrap();
    world.set_connection_group(3, 64, 4, Some(5), true).unwrap();
    world.set_connection_group(3, 64, 4, None, true).unwrap();
    world
        .set_connection_group(33, 64, 1, Some(99), true)
        .unwrap();

    hub.broadcast_block_changes(&mut world);
    assert!(world.connection_group_log.as_ref().unwrap().is_empty());

    assert_eq!(
        group_messages(&decode_packets(&rx_sunrise)),
        vec![Message::Update {
            entries: vec![
                UpdateEntry {
                    pos: [1, 64, 2],
                    group: Some(20),
                },
                UpdateEntry {
                    pos: [3, 64, 4],
                    group: None,
                },
            ],
        }]
    );
    assert!(group_messages(&decode_packets(&rx_vanilla)).is_empty());

    // Bulk chunk resend: if the chunk now has no connection groups, a queued clear still goes
    // out as an incremental Update so the client drops the cleared entry.
    world.set_connection_group(1, 64, 2, None, true).unwrap();
    world.bulk_log.insert((0, 0));
    hub.broadcast_block_changes(&mut world);
    assert_eq!(
        group_messages(&decode_packets(&rx_sunrise)),
        vec![Message::Update {
            entries: vec![UpdateEntry {
                pos: [1, 64, 2],
                group: None,
            }],
        }]
    );
}
