use crate::game::{Game, Result};
use sunrise_client::mobs::MobEvent;

impl Game<'_> {
    pub(in crate::game) fn side_handle_mob_events_reusable(
        &mut self,
        events: &mut Vec<MobEvent>,
    ) -> Result<()> {
        if self.is_logical_server() {
            self.server_handle_mob_events_reusable(events)
        } else {
            self.client_handle_mob_events_reusable(events)
        }
    }
}
