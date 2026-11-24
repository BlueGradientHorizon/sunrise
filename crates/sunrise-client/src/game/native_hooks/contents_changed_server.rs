#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::contents_changed::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::__private::game::Fields;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    pub(in crate::game) fn contents_notice_active(&self, level: LevelRef, pos: BlockPos) -> bool {
        self.server.contents_changing.contains(&(level, pos))
    }
}
