#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::jigsaw_blocks::Pos;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::jigsaw_blocks::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    containers::BlockEntity,
    jigsaw_block::{self, JigsawBlock, JointType},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play;
#[allow(unused_imports)]
pub(in crate::game::server_side) use {
    sunrise_core::nbt::Tag, sunrise_world::worldgen::generator::JigsawQuery,
};

impl Game<'_> {
    /// `handleSetJigsawBlock` for a guest: only a creative player with game
    /// master commands edits the block.
    pub(in crate::game::server_side) fn lan_set_jigsaw_block(
        &mut self,
        conn: u32,
        p: play::SetJigsawBlock,
    ) {
        let Some(g) = self.lan.as_ref().and_then(|host| host.hub.guest(conn)) else {
            return;
        };
        let allowed = g.creative() && g.permission >= 2;
        self.as_guest(conn, |game| {
            if allowed {
                game.apply_set_jigsaw_block(&p);
            }
        });
    }
}
