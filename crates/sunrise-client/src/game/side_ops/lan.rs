//! Role-dispatched LAN side operations (rewritten to `SideOps` traits at switch).
use crate::game::Game;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn side_object_target(
        &self,
        id: i32,
    ) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
        if self.is_logical_server() {
            self.server_object_target(id)
        } else {
            self.client_object_target(id)
        }
    }
}
