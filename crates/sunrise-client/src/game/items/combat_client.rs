use super::Game;
use sunrise_core::random::RandomSource;
use sunrise_level::particle_options::{ParticleOptions, ParticleType};

impl Game<'_, crate::game::client_side::ClientState> {
    /// `LivingEntity.makeDrownParticles`: `count` bubbles around the entity
    /// (vanilla's eight), moving with it.
    pub(in crate::game::client_side) fn drown_particles(
        &mut self,
        at: [f64; 3],
        velocity: [f64; 3],
        count: i32,
    ) {
        assert!(
            !self.is_logical_server(),
            "drown_particles called on Server role"
        );
        let bubble = ParticleOptions::simple(ParticleType::Bubble);
        for _ in 0..count {
            let mut triangle = || {
                let r = self.side.particles.0.random();
                r.next_double() - r.next_double()
            };
            let p = [at[0] + triangle(), at[1] + triangle(), at[2] + triangle()];
            self.side.particles.0.add(&bubble, p, velocity);
        }
    }
    /// Entity event 67 from a server, for this client's own player or an
    /// entity it draws.
    pub(in crate::game::client_side) fn remote_drown(&mut self, id: i32) {
        assert!(
            !self.is_logical_server(),
            "remote_drown called on Server role"
        );
        let own = self.remote.as_ref().is_some_and(|r| r.entity_id == id);
        let body = if own {
            self.player.as_ref().map(|p| {
                let (at, v) = (p.position, p.velocity);
                ([at.x, at.y, at.z], [v.x, v.y, v.z])
            })
        } else {
            self.mobs.iter().find(|m| m.id == id as u32).map(|m| {
                let (at, v) = (m.position, m.velocity);
                ([at.x, at.y, at.z], [v.x, v.y, v.z])
            })
        };
        if let Some((at, v)) = body {
            self.drown_particles(at, v, 8);
        }
    }
}

#[cfg(test)]
#[path = "combat/bee_sting_tests_client.rs"]
pub(in crate::game) mod bee_sting_tests;
