//! Client-side mob entity-event particle bursts (`Entity.handleEntityEvent`).
use super::Game;

impl Game<'_> {
    /// `handleEntityEvent(event)`'s particles about mob `id`.
    pub(in crate::game) fn entity_event_particles(&mut self, id: u32, event: i8) {
        let Some(m) = self.mobs.iter().find(|m| m.id == id) else {
            return;
        };
        if let Some((kind, burst)) =
            sunrise_client::particles::entity_event_burst(&m.kind.id, event as u8)
        {
            self.particles_around_mob(id, kind, burst);
        }
    }
}
