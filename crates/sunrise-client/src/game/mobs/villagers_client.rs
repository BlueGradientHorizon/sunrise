#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::villagers::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    gossip::{self, GossipType},
    items::ItemStack,
    merchant::TradeFeedback,
    mobs::{Mob, Species, entity_sounds::full_id, spawning, villager, villager_brain},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{geometry::Vec3, position::BlockPos};

impl Game<'_> {
    /// `particles::around_self` on the client: the particle engine's random
    /// stands in for the client entity's, so no server draw moves.
    pub(in crate::game) fn particles_around(
        &mut self,
        kind: &'static str,
        feet: [f64; 3],
        size: [f64; 2],
        burst: sunrise_client::particles::Burst,
    ) {
        let Some(ty) = sunrise_client::particles::ParticleType::from_name(kind) else {
            return;
        };
        let options = sunrise_client::particles::ParticleOptions::simple(ty);
        let mut spawned = Vec::new();
        sunrise_client::particles::around_self(
            self.particles.0.random(),
            feet,
            size[0],
            size[1],
            burst,
            |p, v| spawned.push((p, v)),
        );
        for (p, v) in spawned {
            self.particles.0.add(&options, p, v);
        }
    }
}
