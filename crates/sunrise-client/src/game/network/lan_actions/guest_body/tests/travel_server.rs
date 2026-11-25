use crate::game::lan_actions::guest_body::tests::travel::guest;
use sunrise_protocol::packets::{Empty, play::Serverbound as S};

pub(in crate::game) fn send(game: &mut crate::game::ServerGame<'_>, packet: S) {
    game.lan.as_mut().unwrap().hub.update_guest(1, |g| {
        use sunrise_client::lan::guest::InboxItem;
        g.inbox.push_back(InboxItem::Packet(packet, None));
        g.inbox
            .push_back(InboxItem::Packet(S::ClientTickEnd(Empty), None));
    });
    game.side.level_pass.packets_drained = false;
    game.tick_lan_packets();
    // The packet phase holds output until the tick's usual network handoff.
    guest(game).sender.flush();
}
