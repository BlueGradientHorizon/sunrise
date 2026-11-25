#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, maps::MapBanner};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::banners::layer_textures;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::banners::map_banner;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::banners::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    banner::{self, BannerLayer},
    containers::BlockEntity,
};

impl Game<'_> {
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
