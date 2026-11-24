#[allow(unused_imports)]
pub(in crate::game::server_side) use super::super::{
    Game,
    entity::{Entity, with_entity},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pufferfish_player::PUFFER_FISH_STING;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::pufferfish_player::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    mobs::Species, scoreboard::teams_allow_damage,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play::{Clientbound, GameEvent};

impl Game<'_> {
    /// `ServerPlayer.connection.send(ClientboundGameEventPacket(PUFFER_FISH_STING, 0.0F))`: the
    /// stung player's own client plays it (`ClientPacketListener.handleGameEvent`). A server has
    /// no player of its own, so the packet is the only way to reach it.
    pub(in crate::game) fn puffer_fish_sting(&mut self) {
        self.server
            .guest_packets
            .push(Clientbound::GameEvent(GameEvent {
                event: PUFFER_FISH_STING,
                param: 0.0,
            }));
    }
}

#[cfg(test)]
#[path = "pufferfish_sting_tests_server.rs"]
pub(in crate::game) mod pufferfish_sting_tests;
