#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, commands::Target};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::combat::CombatTracker;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::combat::DROWN_EVENT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::combat::Entry;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::combat::PendingAttacker;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::combat::PlayerCredit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::combat::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::health::melee;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    health::{Attack, HealthEvent},
    mobs::{Damage, Mob, Species, Who, stuck::Stuck},
    motion::MotionEvent,
    particles::{ParticleOptions, ParticleType},
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::RandomSource;

impl Game<'_> {
    /// `LivingEntity.makeDrownParticles`: `count` bubbles around the entity
    /// (vanilla's eight), moving with it.
    pub(in crate::game) fn drown_particles(
        &mut self,
        at: [f64; 3],
        velocity: [f64; 3],
        count: i32,
    ) {
        let bubble = ParticleOptions::simple(ParticleType::Bubble);
        for _ in 0..count {
            let mut triangle = || {
                let r = self.particles.0.random();
                r.next_double() - r.next_double()
            };
            let p = [at[0] + triangle(), at[1] + triangle(), at[2] + triangle()];
            self.particles.0.add(&bubble, p, velocity);
        }
    }
}

#[cfg(test)]
#[path = "combat/bee_sting_tests_client.rs"]
pub(in crate::game) mod bee_sting_tests;
