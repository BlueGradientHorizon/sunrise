//! Shared lifecycle wrappers and world-close cleanup shared across server and client roles.
use super::Game;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn finish_world_closed_common(&mut self) {
        self.attachment_indexes.clear();
        self.passages.retire_world();
        self.resident_entities.table.get_mut().reset();
        super::cauldron_interactions::clear();
        self.clear_world_parts();
    }

    pub(in crate::game) fn native_lifecycle_open_started(&mut self) {
        self.side_lifecycle_open_started();
    }

    pub(in crate::game) fn native_lifecycle_world_ready(&mut self) {
        self.side_lifecycle_world_ready();
    }

    pub(in crate::game) fn native_lifecycle_world_closed(&mut self) {
        self.side_lifecycle_world_closed();
    }
}
