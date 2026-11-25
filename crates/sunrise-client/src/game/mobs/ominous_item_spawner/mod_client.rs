#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::ominous_item_spawner::ABOUT_TO_SPAWN;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::ominous_item_spawner::OminousItemSpawner;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::ominous_item_spawner::ROTATION_SPEED;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::ominous_item_spawner::TICKS_SCALING;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::ominous_item_spawner::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    block_entity_render::vertex_light,
    item_render,
    items::ItemStack,
    mesher::Vertex,
    particles::{ParticleOptions, ParticleType},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{geometry::Vec3, random::RandomSource};

impl Game<'_> {
    /// `OminousItemSpawner.addParticles`: one to three ominous spawning
    /// particles from the spawner out to points around it.
    pub(in crate::game) fn ominous_spawner_particles(&mut self, at: [f64; 3]) {
        let options = ParticleOptions::simple(ParticleType::OminousSpawning);
        let r = self.particles.0.random();
        let count = r.next_int_between_inclusive(1, 3).unwrap_or(1);
        let mut flights = Vec::new();
        for _ in 0..count {
            let mut spread = || 0.4 * (r.next_gaussian() - r.next_gaussian());
            let from = [at[0] + spread(), at[1] + spread(), at[2] + spread()];
            flights.push([from[0] - at[0], from[1] - at[1], from[2] - at[2]]);
        }
        for v in flights {
            self.particles.0.add(&options, at, v);
        }
    }
}
