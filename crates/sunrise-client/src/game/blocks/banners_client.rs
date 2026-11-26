use super::Game;
use crate::game::banners::layer_textures;
use sunrise_level::containers::BlockEntity;

impl Game<'_, crate::game::client_side::ClientState> {
    /// The pattern textures and tints of the banner at `pos`, bottom first.
    pub(in crate::game) fn banner_layers_at(&self, pos: [i32; 3]) -> Vec<(String, u32)> {
        let Some(BlockEntity::Banner(b)) = self.world.block_entities.get(&(pos[0], pos[1], pos[2]))
        else {
            return Vec::new();
        };
        layer_textures(&self.stations().banners, &b.layers)
    }
}
