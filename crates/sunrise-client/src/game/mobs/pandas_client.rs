#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game,
    pickups::{Collector, Taken},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::pandas::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    items::ItemStack,
    mobs::{Species, panda},
    particles::{ParticleOptions, ParticleType},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;

impl Game<'_, crate::game::client_side::ClientState> {
    /// What a window draws of its pandas: the crumbs of a bite
    /// (`addEatingParticles`) and the sneeze's particle.
    pub(in crate::game::client_side) fn panda_effects(&mut self) {
        let mut bites = Vec::new();
        let mut sneezes = Vec::new();
        for m in self
            .mobs
            .iter_mut()
            .filter(|m| m.species() == Species::Panda)
        {
            let (crumbs, sneeze) = panda::effects(m);
            if let (Some(crumbs), Some(item)) = (
                crumbs,
                sunrise_client::mobs::finalize::item(m, sunrise_client::mobs::finalize::MAINHAND),
            ) {
                bites.push((item, crumbs));
            }
            sneezes.extend(sneeze);
        }
        for (item, crumbs) in bites {
            let options = self.item_particle(&ItemStack::new(&item, 1));
            for (at, velocity) in crumbs {
                self.side.particles.0.add(&options, at, velocity);
            }
        }
        let options = ParticleOptions::simple(ParticleType::Sneeze);
        for (at, velocity) in sneezes {
            self.side.particles.0.add(&options, at, velocity);
        }
    }
}
