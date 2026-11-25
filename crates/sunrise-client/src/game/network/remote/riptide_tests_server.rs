use super::*;
use crate::game::remote::riptide_tests::OWNER;
use sunrise_client::lan::guest::InboxItem;
use sunrise_protocol::connection::ConnReader;

/// Transfer the client's real play packets to the integrated server's normal
/// connection drain. The test drives ticks without scheduling two game loops.
pub(in crate::game) fn client_tick_to_server(
    client: &crate::game::ClientGame<'_>,
    peer: &mut ConnReader,
    server: &mut crate::game::ServerGame<'_>,
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
    server.side.level_pass = Default::default();
    server.tick_lan_packets();
    let hub = &mut server.lan.as_mut().unwrap().hub;
    hub.end_tick();
    hub.guest(OWNER).unwrap().sender.flush();
    packets
}
