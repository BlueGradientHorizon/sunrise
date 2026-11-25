use super::*;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// Tags changed on this side (NeoForge's `TagsUpdatedEvent`), after they are readable.
    pub(in crate::game) fn native_tags_updated(&mut self, cause: mod_api::TagsUpdateCause) {
        crate::game::native_hooks::fire::fire_tags_updated(self, cause);
    }
}
