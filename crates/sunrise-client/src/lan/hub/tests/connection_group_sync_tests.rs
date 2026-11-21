use super::*;
use crate::{
    lan::viewed_levels::areas::{ChunkArea, ChunkAreaMode},
    world::ConnectionGroupNotify,
};
use sunrise_protocol::{
    buf::Reader,
    connection_groups::{self, ChunkEntry, Message, UpdateEntry},
    packets::play::Clientbound as C,
    viewed_levels,
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

fn wrapped_group_messages(packets: &[C]) -> Vec<(viewed_levels::Level, Message)> {
    let mut out = Vec::new();
    for p in packets {
        let C::CustomPayload(cp) = p else {
            continue;
        };
        if cp.channel != viewed_levels::CHANNEL {
            continue;
        }
        if let Ok(viewed_levels::Message::Run { level, packets }) =
            viewed_levels::Message::decode_in(&cp.data, viewed_levels::REVISION)
        {
            for (id, body) in packets {
                if let Ok(C::CustomPayload(inner)) = C::decode(id, body)
                    && inner.channel == connection_groups::CHANNEL
                {
                    out.push((level, connection_groups::decode(&inner.data).unwrap()));
                }
            }
        }
    }
    out
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
fn incremental_updates_dedup_wire_order_auto_clear_filter_and_bulk_resend() {
    let (mut hub, rx_sunrise, rx_vanilla) = hub_with_sunrise_and_vanilla();
    let mut world = World::new(sunrise_world::registry::BlockStateId(0), 0);
    world.block_log = Some(Vec::new());
    world.connection_group_log = Some(Vec::new());
    world.chunks.insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    world.chunks.insert((2, 0), Arc::new(Chunk::empty(2, 0)));
    hub.guests.update(0, |g| _ = g.sent_chunks.insert((0, 0)));
    hub.guests.update(1, |g| _ = g.sent_chunks.insert((0, 0)));

    // Multiple edits to the same cell collapse to the last value; an unsent chunk (2, 0) is
    // omitted from incremental updates; block updates precede connection-group updates on wire.
    world.block_log.as_mut().unwrap().push((1, 64, 2));
    assert!(world.set_connection_group(1, 64, 2, Some(10), ConnectionGroupNotify::Explicit));
    assert!(world.set_connection_group(1, 64, 2, Some(20), ConnectionGroupNotify::Explicit));
    assert!(world.set_connection_group(3, 64, 4, Some(5), ConnectionGroupNotify::Explicit));
    assert!(world.set_connection_group(3, 64, 4, None, ConnectionGroupNotify::Explicit));
    assert!(world.set_connection_group(33, 64, 1, Some(99), ConnectionGroupNotify::Explicit));

    hub.broadcast_block_changes(&mut world);
    assert!(world.connection_group_log.as_ref().unwrap().is_empty());

    let sunrise = decode_packets(&rx_sunrise);
    let block_idx = sunrise
        .iter()
        .position(|p| matches!(p, C::BlockUpdate(_)))
        .expect("BlockUpdate sent");
    let group_idx = sunrise
        .iter()
        .position(|p| matches!(p, C::CustomPayload(cp) if cp.channel == connection_groups::CHANNEL))
        .expect("sunrise:connection_groups sent");
    assert!(block_idx < group_idx, "block update precedes group update");
    assert_eq!(
        group_messages(&sunrise),
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

    // Automatic clear (`OnBlockSend`) without a paired block send is dropped (and also cancels a
    // superseded earlier explicit write to the same cell in the same pass).
    assert!(world.set_connection_group(1, 64, 2, None, ConnectionGroupNotify::OnBlockSend));
    assert!(world.set_connection_group(4, 64, 4, Some(42), ConnectionGroupNotify::Explicit));
    assert!(world.set_connection_group(4, 64, 4, None, ConnectionGroupNotify::OnBlockSend));
    hub.broadcast_block_changes(&mut world);
    assert!(
        group_messages(&decode_packets(&rx_sunrise)).is_empty(),
        "unsent block replacements never leak connection-group clears"
    );

    // Paired with a sent block update, `OnBlockSend` rides after `BlockUpdate`.
    assert!(world.set_connection_group(1, 64, 2, Some(30), ConnectionGroupNotify::Explicit));
    hub.broadcast_block_changes(&mut world);
    let _ = decode_packets(&rx_sunrise);
    world.block_log.as_mut().unwrap().push((1, 64, 2));
    assert!(world.set_connection_group(1, 64, 2, None, ConnectionGroupNotify::OnBlockSend));
    hub.broadcast_block_changes(&mut world);
    let paired = decode_packets(&rx_sunrise);
    let block_idx = paired
        .iter()
        .position(|p| matches!(p, C::BlockUpdate(_)))
        .expect("BlockUpdate sent");
    let group_idx = paired
        .iter()
        .position(|p| matches!(p, C::CustomPayload(cp) if cp.channel == connection_groups::CHANNEL))
        .expect("paired OnBlockSend clear sent");
    assert!(block_idx < group_idx);
    assert_eq!(
        group_messages(&paired),
        vec![Message::Update {
            entries: vec![UpdateEntry {
                pos: [1, 64, 2],
                group: None,
            }],
        }]
    );

    // Bulk chunk resend: even when the chunk now has no connection groups, emit an authoritative
    // empty `Chunk { replace: true, entries: [] }` snapshot after `LevelChunkWithLight` and
    // suppress incremental clears for that chunk.
    assert!(world.set_connection_group(1, 64, 2, Some(40), ConnectionGroupNotify::Explicit));
    hub.broadcast_block_changes(&mut world);
    let _ = decode_packets(&rx_sunrise);
    assert!(world.set_connection_group(1, 64, 2, None, ConnectionGroupNotify::Explicit));
    world.bulk_log.insert((0, 0));
    hub.broadcast_block_changes(&mut world);
    let bulk_pkts = decode_packets(&rx_sunrise);
    let chunk_idx = bulk_pkts
        .iter()
        .position(|p| matches!(p, C::LevelChunkWithLight(_)))
        .expect("bulk LevelChunkWithLight sent");
    let group_idx = bulk_pkts
        .iter()
        .position(|p| matches!(p, C::CustomPayload(cp) if cp.channel == connection_groups::CHANNEL))
        .expect("authoritative empty Chunk snapshot sent");
    assert!(chunk_idx < group_idx);
    assert_eq!(
        group_messages(&bulk_pkts),
        vec![Message::Chunk {
            cx: 0,
            cz: 0,
            replace: true,
            entries: Vec::new(),
        }]
    );
}

#[test]
fn chunk_snapshot_splits_above_max_entries_with_replace_only_on_first() {
    let mut chunk = Chunk::empty(0, 0);
    let total = connection_groups::MAX_ENTRIES + 1;
    for i in 0..total {
        let lx = (i & 15) as i32;
        let lz = ((i >> 4) & 15) as i32;
        let y = (i >> 8) as i32;
        chunk.set_connection_group(lx, y, lz, Some(i as u64));
    }
    let bodies = block_changes::encode_chunk_connection_groups(&chunk, false);
    assert_eq!(bodies.len(), 2);
    let decoded: Vec<Message> = bodies
        .iter()
        .map(|body| {
            let mut r = Reader::new(body);
            let id = r.varint().unwrap();
            let C::CustomPayload(cp) = C::decode(id, r.rest()).unwrap() else {
                panic!("not custom payload");
            };
            assert_eq!(cp.channel, connection_groups::CHANNEL);
            connection_groups::decode(&cp.data).unwrap()
        })
        .collect();
    match &decoded[0] {
        Message::Chunk {
            cx: 0,
            cz: 0,
            replace: true,
            entries,
        } => assert_eq!(entries.len(), connection_groups::MAX_ENTRIES),
        other => panic!("unexpected first part: {other:?}"),
    }
    match &decoded[1] {
        Message::Chunk {
            cx: 0,
            cz: 0,
            replace: false,
            entries,
        } => assert_eq!(entries.len(), 1),
        other => panic!("unexpected second part: {other:?}"),
    }
}

#[test]
fn viewed_level_syncs_initial_snapshots_updates_and_bulk_resends_to_negotiated_viewers_only() {
    let (mut hub, rx_sunrise, rx_vanilla) = hub_with_sunrise_and_vanilla();
    hub.ticks = 10;
    let levels = hub.level_keys();
    let answer = viewed_levels::Answer {
        revision: viewed_levels::REVISION,
        max_levels: 2,
    };
    let uuid0 = hub.guests[0].uuid;
    let uuid1 = hub.guests[1].uuid;
    for idx in 0..2 {
        hub.guests.update(idx, |g| {
            g.sender.viewed_levels = answer;
            g.batch_quota = 9.0;
            g.viewed.logged_in_with(levels.clone());
        });
    }
    let area = vec![ChunkArea {
        level: "minecraft:the_nether".into(),
        center: (0, 0),
        radius: 0,
        mode: ChunkAreaMode::Loaded,
    }];
    hub.set_viewed_areas(uuid0, "test:portal", area.clone())
        .unwrap();
    hub.set_viewed_areas(uuid1, "test:portal", area).unwrap();
    hub.set_dimension("minecraft:the_nether");

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
    let mut run_pass = |hub: &mut Hub, world: &mut World| {
        let mut ctx = TickCtx {
            sea_level: 63,
            world,
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
        assert!(hub.viewed_pass(&mut ctx));
    };

    // 1. Initial viewed-level chunk batch: snapshot arrives wrapped after ChunkBatchFinished,
    //    only for the negotiated Sunrise guest.
    run_pass(&mut hub, &mut world);
    let sunrise = decode_packets(&rx_sunrise);
    let batch_end = sunrise
        .iter()
        .position(|p| matches!(p, C::ChunkBatchFinished(_)))
        .expect("ChunkBatchFinished sent");
    let group_run = sunrise
        .iter()
        .rposition(|p| !wrapped_group_messages(std::slice::from_ref(p)).is_empty())
        .expect("wrapped sunrise:connection_groups sent");
    assert!(batch_end < group_run);
    let wrapped = wrapped_group_messages(&sunrise);
    assert_eq!(wrapped.len(), 1);
    assert_eq!(
        wrapped[0].1,
        Message::Chunk {
            cx: 0,
            cz: 0,
            replace: true,
            entries: vec![ChunkEntry {
                local_x: 3,
                y: 64,
                local_z: 5,
                group: 77,
            }],
        }
    );
    let vanilla = decode_packets(&rx_vanilla);
    assert!(
        vanilla
            .iter()
            .any(|p| matches!(p, C::ChunkBatchFinished(_)))
    );
    assert!(wrapped_group_messages(&vanilla).is_empty());

    // 2. Incremental update in a viewed level routes wrapped to negotiated viewers only.
    assert!(world.set_connection_group(3, 64, 5, Some(88), ConnectionGroupNotify::Explicit));
    run_pass(&mut hub, &mut world);
    let wrapped = wrapped_group_messages(&decode_packets(&rx_sunrise));
    assert_eq!(wrapped.len(), 1);
    assert_eq!(
        wrapped[0].1,
        Message::Update {
            entries: vec![UpdateEntry {
                pos: [3, 64, 5],
                group: Some(88),
            }],
        }
    );
    assert!(wrapped_group_messages(&decode_packets(&rx_vanilla)).is_empty());

    // 3. Bulk resend in a viewed level sends an authoritative empty snapshot wrapped.
    assert!(world.set_connection_group(3, 64, 5, None, ConnectionGroupNotify::Explicit));
    world.bulk_log.insert((0, 0));
    run_pass(&mut hub, &mut world);
    let wrapped = wrapped_group_messages(&decode_packets(&rx_sunrise));
    assert_eq!(wrapped.len(), 1);
    assert_eq!(
        wrapped[0].1,
        Message::Chunk {
            cx: 0,
            cz: 0,
            replace: true,
            entries: Vec::new(),
        }
    );
    assert!(wrapped_group_messages(&decode_packets(&rx_vanilla)).is_empty());
}
