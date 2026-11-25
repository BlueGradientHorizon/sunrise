use super::Game;
use sunrise_client::{
    blocks::RenderShape,
    mobs::{Species, breeze},
    particles::{ParticleOptions, ParticleType},
};

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn tick_breeze_effects(&mut self) {
        let Some(camera) = self.player.as_ref().map(|p| p.position) else {
            return;
        };
        for i in 0..self.mobs.len() {
            let m = &self.mobs[i];
            if m.species() != Species::Breeze
                || !m.alive()
                || m.position.distance_squared(camera) > 32.0 * 32.0
            {
                continue;
            }
            let pose = breeze::pose(m);
            let (amount, jump_trail) = match pose {
                breeze::SLIDING => (20, false),
                breeze::LONG_JUMPING => (3, true),
                _ => (1, false),
            };
            if jump_trail {
                self.mobs[i].extra.scratch[1] += 1;
                if self.mobs[i].extra.scratch[1] > 5 {
                    continue;
                }
            } else {
                self.mobs[i].extra.scratch[1] = 0;
            }
            let (block, state) = self.breeze_ground(&self.mobs[i]);
            // `emitGroundParticles` skips an invisible block; the jump trail
            // (`emitJumpTrailParticles`) does not ask.
            if !jump_trail && self.res.facts().get(state).render_shape == RenderShape::Invisible {
                continue;
            }
            let Some((sprite, tint)) = self.particle_sprite(state, block) else {
                continue;
            };
            let m = &self.mobs[i];
            let at = if jump_trail {
                [
                    m.position.x + m.velocity.x,
                    m.position.y + m.velocity.y + f64::from(0.1_f32),
                    m.position.z + m.velocity.z,
                ]
            } else {
                [m.position.x, m.position.y, m.position.z]
            };
            // `ClientLevel.addParticle` with no speed: each crumb's own
            // generator draws its lifetime, its little push and the quarter of
            // the block's sprite it shows, never the level's random.
            let options = ParticleOptions::block(ParticleType::Block, sprite, tint);
            for _ in 0..amount {
                self.side.particles.0.add(&options, at, [0.0; 3]);
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use crate::game::{Audio, Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_client::chunk::Chunk;
    use sunrise_client::mobs::Mob;
    use sunrise_core::geometry::Vec3;

    /// A game on the prepared resources with an empty chunk at (0, 0), the
    /// window's player (the camera the dust needs) and one breeze standing
    /// with its feet at `feet`.
    pub(in crate::game) fn game_with_breeze<'a>(
        resources: &'a mut Option<std::sync::Arc<crate::game::Resources>>,
        feet: Vec3,
    ) -> crate::game::ClientGame<'a> {
        let root = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES")
                .expect("set SUNRISE_TEST_RESOURCES to prepared resources"),
        );
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::Client,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = crate::game::execution_role::ExecutionRole::Client;
        let res = &*resources.insert(loaded.resources);
        let mut game = crate::game::ClientGame::new(
            res,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.clone(),
            )),
            None,
        )
        .unwrap();
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        // Eight blocks south of the breeze: inside the 32 blocks the dust needs.
        game.player = Some(
            sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::new(8.5, 66.0, 16.5))
                .unwrap(),
        );
        game.side.particles.0.camera = [8.5, 66.0, 16.5];
        let kind = game.res.mob_types()["minecraft:breeze"].clone();
        game.mobs.push(Mob::new(13001, kind, feet, 0.0));
        game
    }

    pub(in crate::game) fn made_block_particles(game: &crate::game::ClientGame<'_>) -> u64 {
        game.side
            .particles
            .0
            .made()
            .get("block")
            .copied()
            .unwrap_or(0)
    }
}
