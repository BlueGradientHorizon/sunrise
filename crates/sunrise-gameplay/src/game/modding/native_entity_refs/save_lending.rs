//! Save visibility while authoritative rows are lent out.

impl crate::game::ServerGame<'_> {
    /// Keep a held-out entity inaccessible until its copy has been stored back.
    pub fn with_entity_held_out<R>(&mut self, id: u32, action: impl FnOnce(&mut Self) -> R) -> R {
        let _save_lease = sunrise_sim::join_seq::visibility::PassLease::enter();
        if mod_api::__private::mods().is_empty() {
            return action(self);
        }
        self.server.held_out.push(id);
        let result = action(self);
        self.server.held_out.pop();
        result
    }
}
