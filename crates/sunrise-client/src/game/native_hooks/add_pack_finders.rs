use super::*;

impl Game<'_> {
    /// Register sources when a resource or data pack repository is discovered.
    pub(in crate::game) fn native_add_pack_finders(
        &mut self,
        packs: &mut dyn mod_api::PackFinders,
    ) {
        if self.mods.has_unapplied_code_switches() {
            let mods = self.mods.clone();
            let host = execution_role::host_side(self);
            mod_api::__private::dispatch_add_pack_finders_for(host, packs, |id| {
                mods.code_owner_enabled(id)
            });
            return;
        }
        self.fire_add_pack_finders(packs);
    }
}
