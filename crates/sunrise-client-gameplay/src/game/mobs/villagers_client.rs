//! Client-side mob entity-event particle bursts (`Entity.handleEntityEvent`).
use crate::game::ClientGame;
use sunrise_level::particle_options::{
    ParticleOptions, ParticleType, around_self, entity_event_burst,
};

impl ClientGame<'_> {
    /// `handleEntityEvent(event)`'s particles about mob `id`.
    pub(in crate::game) fn entity_event_particles(&mut self, id: u32, event: i8) {
        let Some(m) = self.common.mobs.iter().find(|m| m.id == id) else {
            return;
        };
        let Some((kind, burst)) = entity_event_burst(&m.kind.id, event as u8) else {
            return;
        };
        let feet = [m.position.x, m.position.y, m.position.z];
        let size = [f64::from(m.width()), f64::from(m.height())];
        let Some(ty) = ParticleType::from_name(kind) else {
            return;
        };
        let options = ParticleOptions::simple(ty);
        let mut spawned = Vec::new();
        let particles = &mut self.client.particles.0;
        around_self(particles.random(), feet, size[0], size[1], burst, |p, v| {
            spawned.push((p, v));
        });
        for (p, v) in spawned {
            particles.add(&options, p, v);
        }
    }
}
