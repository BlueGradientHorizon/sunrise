

#[cfg(any(test, feature = "test-support"))]
pub mod player_status_tests {
    use super::Game;
    use std::sync::Arc;
    use sunrise_core::geometry::Vec3;
    use sunrise_level::{chunk::Chunk, effects::EffectInstance};
    use sunrise_sim::motion::PlayerMotion;

    /// The window's player standing at (0.5, 64, 0.5) on the empty chunks around it
    /// (`EntityFluidInteraction.update` needs the chunks around the box loaded as well).
    pub fn stand_player<S: crate::game::Side>(game: &mut Game<'_, S>)
    where
        for<'__g> crate::game::Game<'__g, S>: ::mod_api::Host,
    {
        for chunk in [(-1, -1), (-1, 0), (0, -1), (0, 0)] {
            game.world
                .chunks
                .insert(chunk, Arc::new(Chunk::empty(chunk.0, chunk.1)));
        }
        game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
    }

    /// Plains in every quart `BiomeManager.getBiome` can pick for the player's feet and head
    /// blocks, (0, 64, 0) and (0, 65, 0): the eight corners of the quart around (block - 2),
    /// (-1, 15, -1), which reach into the three neighbouring chunks. Then rain starts (`true`)
    /// or stops.
    pub fn rain_on_the_player<S: crate::game::Side>(
        game: &mut Game<'_, S>,
        raining: bool,
    ) where
        for<'__g> crate::game::Game<'__g, S>: ::mod_api::Host,
    {
        let plains = game.res.biomes().id("minecraft:plains");
        let bottom = game.world.bottom_y() >> 2;
        for x in [-1, 0] {
            for y in [15, 16] {
                for z in [-1, 0] {
                    let chunk = game.world.chunks.get_mut(&(x >> 2, z >> 2)).unwrap();
                    Arc::make_mut(chunk).set_biome_cell(x, y - bottom, z, plains);
                }
            }
        }
        game.world.weather.raining = raining;
        game.world.weather.settle_levels();
    }

    

    
}