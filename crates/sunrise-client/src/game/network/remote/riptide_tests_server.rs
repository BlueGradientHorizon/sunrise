#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::riptide_tests::OWNER;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::riptide_tests::START;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::riptide_tests::arena;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::riptide_tests::server_to_client;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::riptide_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{
    native_entity_refs::tests::with_client_game, native_guest_turn_tests::with_guests,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::mpsc::{Receiver, channel};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::lan::{Out, PlayerSender, guest::InboxItem};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::{buf::Reader, connection::ConnReader};

/// Transfer the client's real play packets to the integrated server's normal
/// connection drain. The test drives ticks without scheduling two game loops.
pub(in crate::game) fn client_tick_to_server(
    client: &Game<'_>,
    peer: &mut ConnReader,
    server: &mut Game<'_>,
) -> Vec<S> {
    client.send(S::ClientTickEnd(Empty));
    let mut packets = Vec::new();
    loop {
        let (id, bytes) = peer.recv().unwrap();
        let packet = S::decode(id, &bytes).unwrap();
        let end = matches!(packet, S::ClientTickEnd(_));
        packets.push(packet);
        if end {
            break;
        }
    }
    let hub = &mut server.lan.as_mut().unwrap().hub;
    hub.update_guest(OWNER, |g| {
        g.inbox
            .extend(packets.iter().cloned().map(|p| InboxItem::Packet(p, None)));
    })
    .unwrap();
    server.server.level_pass = Default::default();
    server.tick_lan_packets();
    let hub = &mut server.lan.as_mut().unwrap().hub;
    hub.end_tick();
    hub.guest(OWNER).unwrap().sender.flush();
    packets
}
