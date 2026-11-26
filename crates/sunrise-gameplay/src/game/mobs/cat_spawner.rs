//! `CatSpawner`: every 1200 ticks a cat may appear near a random player, in a village with enough occupied beds or inside a swamp hut.
use super::Dim;

pub(in crate::game) use sunrise_gameplay::cat_spawner::CatSpawner;

impl crate::game::ServerGame<'_> {
    /// `CatSpawner.tick` (the overworld's custom spawner).
    pub(in crate::game::server_side) fn tick_cat_spawner(&mut self) {
        if self.common.dim != Dim::Overworld || !self.server.cat.due() {
            return;
        }
        sunrise_gameplay::cat_spawner::spawn_near_player(self);
    }
    /// `spawnCat`: forwards to the shared flow; only the tests call it.
    #[cfg(test)]
    fn spawn_cat(&mut self, pos: [i32; 3], persistent: bool) {
        sunrise_gameplay::cat_spawner::spawn_cat(self, pos, persistent);
    }
}
