use super::Game;
use sunrise_client::containers::BlockEntity;

impl Game<'_> {
    pub(in crate::game) fn crafter_open(&self) -> bool {
        matches!(self.container_kind(), Some(BlockEntity::Crafter(_)))
    }
}
