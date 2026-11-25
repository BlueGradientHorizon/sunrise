#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, view_vector};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::spear::OWN;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::spear::Target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::spear::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::spear::hits_in_reach;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    attack_strength::delay,
    collisions::SceneCollisions,
    first_person::Hand,
    items::ItemStack,
    mobs::Damage,
    spear::{HIT_FEEDBACK_TICKS, Kinetic, REACH},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3, traverse_blocks},
    position::BlockPos,
};

impl Game<'_> {
    /// A sound at the player's feet.
    pub(in crate::game::client_side) fn client_play_at_feet(&mut self, sound: &str) {
        if let Some(p) = &self.player {
            let at = [p.position.x, p.position.y, p.position.z];
            self.audio.0.play(sound, Some(at), 1.0, 1.0);
        }
    }
}
