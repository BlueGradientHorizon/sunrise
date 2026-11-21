use super::*;

impl Game<'_> {
    /// Register sources when a resource or data pack repository is discovered.
    pub(in crate::game) fn native_add_pack_finders(
        &mut self,
        packs: &mut crate::game::pack_finders::Registry,
    ) {
        self.fire_add_pack_finders(packs);
    }
}
