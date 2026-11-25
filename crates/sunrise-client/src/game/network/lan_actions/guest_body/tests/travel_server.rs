#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::execution_role::ExecutionRole;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::ACCEPTED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::MoveVeto;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::NEXT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::Reply;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::START;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::acknowledge;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::correction;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::guest;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::packets;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::report;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::with_player;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::with_reply;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::guest_body::tests::travel::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::{
    fixture_root, with_game_role,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_item_places::tests::guest as online_guest;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    EntityMoveEvent, Flow, Hooks, LivingTravelEvent, LoopContext, MoveCause,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::atomic::{AtomicUsize, Ordering};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    chunk::Chunk,
    lan::{NetConfig, Out, hub::HubConfig},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::{Empty, play::Serverbound as S};

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
