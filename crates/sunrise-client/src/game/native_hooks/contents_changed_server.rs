use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn contents_notice_active(&self, level: LevelRef, pos: BlockPos) -> bool {
        self.side.contents_changing.contains(&(level, pos))
    }
}
