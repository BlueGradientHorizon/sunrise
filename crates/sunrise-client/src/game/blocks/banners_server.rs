use super::{Game, maps::MapBanner};
use crate::game::banners::map_banner;

impl Game<'_, crate::game::server_side::ServerState> {
    /// The banner at `pos` as a map marker source (`MapBanner.fromWorld`).
    pub(in crate::game::server_side) fn map_banner_at(&self, pos: [i32; 3]) -> Option<MapBanner> {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        map_banner(
            pos,
            self.world.block_entities.get(&(pos[0], pos[1], pos[2])),
            &self.res.facts().get(state).name,
        )
    }
}
