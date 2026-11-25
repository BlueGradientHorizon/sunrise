//! Breeze particles (`Breeze.tick`): block dust at its feet while it stands,
//! a burst of twenty while it slides and a short trail along a long jump.
#[allow(unused_imports)]
use super::{BlockStateId, Game};
#[allow(unused_imports)]
use sunrise_client::{
    blocks::RenderShape,
    mobs::{Mob, Species, breeze},
    particles::{ParticleOptions, ParticleType},
};

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// `Breeze.emitGroundParticles`'s block: `getInBlockState`, else
    /// `getBlockStateOn`, which is `Entity.getOnPos(1.0E-5F)` — the supporting
    /// block the feet rest on, or the cell just below them. A fence, a wall and
    /// a fence gate have a 1.5-block collision, so a breeze on one has its feet
    /// half a block above that block's own cell, where the cell below them is
    /// air and only the supporting block has the material.
    pub(in crate::game) fn breeze_ground(&self, m: &Mob) -> ([i32; 3], BlockStateId) {
        let feet = [
            m.position.x.floor() as i32,
            m.position.y.floor() as i32,
            m.position.z.floor() as i32,
        ];
        let in_block = self.world.block(feet[0], feet[1], feet[2]);
        if !self.res.facts().is_air(in_block) {
            return (feet, in_block);
        }
        let at = m.ground.supporting_block.map_or(
            [
                feet[0],
                (m.position.y - f64::from(1.0e-5_f32)).floor() as i32,
                feet[2],
            ],
            |p| [p.x, p.y, p.z],
        );
        (at, self.world.block(at[0], at[1], at[2]))
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::game::{Audio, Options, Progress, Settings, entity::Entity, load_resources};
    #[allow(unused_imports)]
    use std::sync::Arc;
    #[allow(unused_imports)]
    use sunrise_client::chunk::Chunk;
    #[allow(unused_imports)]
    use sunrise_client::mobs::Mob;
    #[allow(unused_imports)]
    use sunrise_core::geometry::Vec3;
    #[allow(unused_imports)]
    use sunrise_core::random::LegacyRandom;

    #[allow(unused_imports)]
    pub(in crate::game) use crate::game::client_side::breezes_client::tests::game_with_breeze;

    /// A breeze standing on a fence rests its feet at 64 + 1.5, where its own
    /// cell is air: `Breeze.emitGroundParticles` asks `getBlockStateOn` for the
    /// supporting block, so the crumbs are the fence's. Every one of them is
    /// made by the particle's own generator (`Particle.random`), so the
    /// client level's random is where it was before the tick.
    #[test]
    #[ignore = "daily"]
    fn the_dust_of_a_breeze_on_a_fence_keeps_the_client_level_random() {
        let mut resources = None;
        let mut game = game_with_breeze(&mut resources, Vec3::new(8.5, 65.5, 8.5));
        let registry = &game.pack.assets.registry;
        let stone = registry.default_state("minecraft:stone").unwrap();
        let fence = registry.default_state("minecraft:oak_fence").unwrap();
        game.world.set(8, 63, 8, stone).unwrap();
        game.world.set(8, 64, 8, fence).unwrap();
        let _ = game.mobs[0].update_retained_ground(
            &game.world,
            game.collision,
            &world_items!(game).entity_type_tags,
            true,
            None,
        );
        assert_eq!(
            game.breeze_ground(&game.mobs[0]),
            ([8, 64, 8], fence),
            "the feet's own cell is air; the fence is what holds them up"
        );
        game.world.random = sunrise_core::random::LevelRandom::for_level(LegacyRandom::new(1234));
        let level = game.world.random.state();

        game.tick_breeze_effects();

        assert_eq!(
            game.particles.0.made().get("block").copied().unwrap_or(0),
            1,
            "vanilla emits one crumb of the fence the breeze stands on"
        );
        assert_eq!(
            game.world.random.state(),
            level,
            "the crumb's constructor draws belong to the particle, not the level"
        );
    }

    /// The control: a breeze standing on a full block's top also crumbles it,
    /// and its constructor's draws still leave the client level's random alone.
    #[test]
    #[ignore = "daily"]
    fn the_dust_of_a_breeze_on_a_full_block_keeps_the_client_level_random() {
        let mut resources = None;
        let mut game = game_with_breeze(&mut resources, Vec3::new(8.5, 64.0, 8.5));
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        game.world.set(8, 63, 8, stone).unwrap();
        let _ = game.mobs[0].update_retained_ground(
            &game.world,
            game.collision,
            &world_items!(game).entity_type_tags,
            true,
            None,
        );
        game.world.random = sunrise_core::random::LevelRandom::for_level(LegacyRandom::new(1234));
        let level = game.world.random.state();

        game.tick_breeze_effects();

        assert_eq!(
            game.world.random.state(),
            level,
            "the crumb's constructor draws belong to the particle, not the level"
        );
        assert_eq!(
            game.particles.0.made().get("block").copied().unwrap_or(0),
            1
        );
    }
}
