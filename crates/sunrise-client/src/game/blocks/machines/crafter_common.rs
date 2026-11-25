use super::Game;
use sunrise_client::containers::{BlockEntity, Crafter};

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn crafter_open(&self) -> bool {
        matches!(self.container_kind(), Some(BlockEntity::Crafter(_)))
    }
    pub(in crate::game) fn crafter_at(&self, pos: (i32, i32, i32)) -> Option<&Crafter> {
        match self.world.block_entities.get(&pos)? {
            BlockEntity::Crafter(c) => Some(c),
            _ => None,
        }
    }
}
