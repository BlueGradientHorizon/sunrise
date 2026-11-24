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
pub(in crate::game::server_side) use sunrise_client::{
    hud_overlays::Titles, lan::data::component_nbt, text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play::{self, Clientbound as C};

impl Game<'_> {
    /// TitleCommand's `connection.send` for each player: a guest is sent the
    /// packet, the window's own player shows it.
    pub(in crate::game) fn titles_or_packet(
        &mut self,
        players: &[Target],
        local: impl FnOnce(&mut Titles),
        packet: C,
    ) {
        let mut local = Some(local);
        for &t in players {
            match t {
                Target::Player if self.server.lan_acting.is_some() => {
                    self.server.guest_packets.push(packet.clone());
                }
                Target::Player => {
                    if let Some(local) = local.take() {
                        self.titles_or_packet_client(local);
                    }
                }
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
