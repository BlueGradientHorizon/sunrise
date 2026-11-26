//! Client-side mob entity-event particle bursts (`Entity.handleEntityEvent`).
use crate::game::ClientGame;

impl ClientGame<'_> {
    /// `handleEntityEvent(event)`'s particles about mob `id`.
    pub(in crate::game) fn entity_event_particles(&mut self, id: u32, event: i8) {
        let Some(m) = self.common.mobs.iter().find(|m| m.id == id) else {
            return;
        };
        if let Some((kind, burst)) =
            sunrise_level::particle_options::entity_event_burst(&m.kind.id, event as u8)
        {
            self.common.particles_around_mob(id, kind, burst);
        }
    }
}
