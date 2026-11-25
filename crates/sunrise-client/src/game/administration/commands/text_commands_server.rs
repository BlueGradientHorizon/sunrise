#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Arg, Context, Game, R, Target, err};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::text_commands::compile_selector;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::text_commands::invalid_selector;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::text_commands::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{lan::data::component_nbt, text::Text};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play::{self, Clientbound as C};

impl Game<'_, crate::game::server_side::ServerState> {
    /// TitleCommand's `connection.send` for each player: a guest is sent the
    /// packet, the acting player queues it on `server.guest_packets`.
    pub(in crate::game::server_side) fn titles_or_packet(&mut self, players: &[Target], packet: C) {
        assert!(
            self.is_logical_server(),
            "titles_or_packet called on Client role"
        );
        for &t in players {
            match t {
                Target::Player if self.side.lan_acting.is_some() || self.lan.is_none() => {
                    self.side.guest_packets.push(packet.clone());
                }
                Target::Player => {}
                Target::Guest(conn) => {
                    if let Some(host) = self.lan.as_ref() {
                        host.hub.send_to(conn, &packet);
                    }
                }
                _ => {}
            }
        }
    }
}
