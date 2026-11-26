// Test module extracted without changing its logical path.
use std::sync::{Arc, Mutex};

/// Denies griefing to the entities in `self.0`, and logs who was asked.
pub struct Deny(
    pub(in crate::game) Vec<u32>,
    pub(in crate::game) Arc<Mutex<Vec<(u32, bool)>>>,
);
#[mod_api::__subscriptions]
impl mod_api::Hooks for Deny {
    fn entity_mob_griefing(
        &self,
        _: &dyn mod_api::Host,
        event: &mut mod_api::EntityMobGriefingEvent,
    ) {
        self.1
            .lock()
            .unwrap()
            .push((event.entity.id(), event.can_grief));
        if self.0.contains(&event.entity.id()) {
            event.can_grief = false;
        }
    }
}
