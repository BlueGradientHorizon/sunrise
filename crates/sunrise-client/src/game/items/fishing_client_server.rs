use super::Game;
use sunrise_client::particles::ParticleType;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `ServerLevel.sendParticles` of a simple particle.
    pub(in crate::game) fn send_particles(
        &mut self,
        kind: ParticleType,
        at: [f64; 3],
        spread: [f64; 3],
        speed: f64,
        count: u32,
    ) {
        self.server_send_particles(crate::lan::server_effects::ServerQueuedParticle {
            dimension: self.dim.id(),
            particle: crate::lan::server_effects::ServerParticle::simple(kind),
            at,
            spread,
            speed: [speed; 3],
            count: count as i32,
            randomization: 0,
            except: None,
        });
    }
}
