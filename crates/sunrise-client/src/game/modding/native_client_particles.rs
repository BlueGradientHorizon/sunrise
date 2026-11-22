//! Local particle and firework requests from native client hooks.
use super::{
    Game,
    api_values::{TryIntoApi, TryIntoEngine, block_state},
};
use mod_api::{FireworksRequest, ParticleOptions};
use sunrise_client::particles::{ParticleEngine, ParticleLevel, ParticleSpawnFlags};

/// The most explosions one `create_fireworks` request draws (a crafted rocket holds at
/// most 7 stars; a mod's show can be larger).
const MAX_FIREWORK_EXPLOSIONS: usize = 256;

/// Lend game-owned value conversions to the window and scene particle engines.
pub(super) fn configure_particle_inputs(engine: &mut ParticleEngine) {
    engine.set_block_state_source(|| {
        block_state::current()
            .ok()
            .map(|states| -> sunrise_client::particles::BlockStates { states })
    });
    engine.options_to_api = Some(|options| options.try_into_api().ok());
}

impl Game<'_> {
    /// `ClientLevel.addParticle`: request one particle in this client's loaded world.
    /// Options and provider input slots pass through unchanged, including encoded
    /// color or size values in `velocity`. Normal distance, settings and population
    /// limits still apply. True means submitted, not necessarily visible: an unknown
    /// provider or missing sprite set draws nothing. False on a server, without a
    /// client player/level, or for non-finite numeric input. API @API:client_particles@.
    pub(super) fn add_particle(
        &mut self,
        options: &ParticleOptions,
        at: [f64; 3],
        velocity: [f64; 3],
    ) -> bool {
        if !self.native_window()
            || self.player.is_none()
            || !at.into_iter().all(f64::is_finite)
            || !velocity.into_iter().all(f64::is_finite)
        {
            return false;
        }
        let Ok(engine_options) = options.try_into_engine(self.items()) else {
            return false;
        };
        // The limiter's roll draws the level random: inside a part's `animate_tick`, the one
        // the part holds as `event.random`.
        self.with_lent_level_random(|game| {
            let level = ParticleLevel::new(&game.world, game.res.facts(), game.collision);
            game.particles.add_with_level(
                &engine_options,
                at,
                velocity,
                ParticleSpawnFlags::default(),
                &level,
                Some(options),
            );
        });
        true
    }
    /// `ClientLevel.createFireworks`: queue a firework's explosions in this client's loaded
    /// world through the starter a bursting rocket uses, its blast and twinkle sounds gated by
    /// `audible` as a rocket's by its silence. No entity, no damage, nothing on the server.
    /// True means queued; false on a server, without a client player/level, for non-finite
    /// input or for more than [`MAX_FIREWORK_EXPLOSIONS`] explosions. API @API:client_fireworks@.
    pub(super) fn create_fireworks(&mut self, request: &FireworksRequest) -> bool {
        if !self.native_window()
            || self.player.is_none()
            || !request
                .at
                .into_iter()
                .chain(request.velocity)
                .all(f64::is_finite)
            || request.explosions.len() > MAX_FIREWORK_EXPLOSIONS
        {
            return false;
        }
        let Ok(explosions) = request
            .explosions
            .iter()
            .map(|explosion| explosion.try_into_engine(&()))
            .collect();
        let camera = self.eye_position().unwrap_or(request.at);
        // Without explosions its poofs are added at once, each with the limiter's roll.
        self.with_lent_level_random(|game| {
            game.particles.fireworks(
                request.at,
                request.velocity,
                explosions,
                camera,
                request.audible,
            );
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{ClientHost, FireworkExplosion, FireworkShape};
    use sunrise_client::particles::ParticleType;

    /// A client mod's fireworks burst either way, and blast and twinkle only when audible,
    /// as a rocket that isn't silent; a server draws nothing.
    #[test]
    #[ignore = "daily"]
    fn a_client_mods_fireworks_sound_only_when_audible() {
        use crate::game::native_entity_refs::tests::with_client_game;
        with_client_game(|game| {
            let eye = sunrise_core::geometry::Vec3::new(0.5, 200.0, 0.5);
            game.player =
                Some(sunrise_client::motion::PlayerMotion::new(game.motion, eye).unwrap());
            let at = [6.5, 201.5, 10.5];
            let request = |audible| FireworksRequest {
                at,
                velocity: [0.0, 0.05, 0.0],
                explosions: vec![FireworkExplosion {
                    shape: FireworkShape::Star,
                    colors: vec![0xFF5555, 0xFFFF55],
                    fade_colors: vec![0xFFFFFF],
                    trail: true,
                    twinkle: true,
                }],
                audible,
            };
            for (audible, expected) in [
                (
                    true,
                    vec![
                        "minecraft:entity.firework_rocket.blast",
                        "minecraft:entity.firework_rocket.twinkle",
                    ],
                ),
                (false, vec![]),
            ] {
                game.particles.clear();
                game.particles.sounds.clear();
                let host: &mut dyn ClientHost = game;
                assert!(host.create_fireworks(&request(audible)));
                for _ in 0..20 {
                    let level = ParticleLevel {
                        world: &game.world,
                        facts: game.res.facts(),
                        collision: game.collision,
                        player: None,
                        cached_chunk: std::cell::Cell::new(None),
                    };
                    game.particles.tick(&level);
                }
                let heard: Vec<_> = game
                    .particles
                    .sounds
                    .iter()
                    .filter(|s| s.0.starts_with("minecraft:entity.firework_rocket."))
                    .map(|s| (s.0, s.1))
                    .collect();
                let expected: Vec<_> = expected.into_iter().map(|s| (s, at)).collect();
                assert_eq!(heard, expected, "audible {audible}");
                assert!(
                    !game.particles.is_empty(),
                    "the sparks drew, audible {audible}"
                );
            }
            let host: &mut dyn ClientHost = game;
            let mut bad = request(true);
            bad.velocity[1] = f64::NAN;
            assert!(!host.create_fireworks(&bad), "non-finite input");
            let mut most = request(false);
            most.explosions = vec![most.explosions[0].clone(); 256];
            assert!(
                host.create_fireworks(&most),
                "256 explosions, vanilla's most"
            );
            bad = request(false);
            bad.explosions = vec![bad.explosions[0].clone(); 257];
            assert!(!host.create_fireworks(&bad), "257 explosions");
            game.options.execution_role =
                crate::game::execution_role::ExecutionRole::IntegratedServer;
            let host: &mut dyn ClientHost = game;
            assert!(
                !host.create_fireworks(&request(true)),
                "servers draw nothing"
            );
        });
    }
}
