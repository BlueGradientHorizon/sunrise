#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::sulfur_cubes::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    items::ItemStack,
    mesher::Vertex,
    mobs::{Mob, Species, Who, sulfur_cube},
    projectiles::Projectile,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::BlockPos;

impl Game<'_> {
    pub(in crate::game) fn sulfur_sound(&mut self, index: usize, sound: &str) {
        let at = self.mobs[index].center();
        self.server_play_sound(&format!("minecraft:{sound}"), at, 1.0, 1.0);
    }
}
