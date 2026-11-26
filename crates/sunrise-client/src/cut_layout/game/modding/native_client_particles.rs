#![cfg(test)]
#[cfg(test)]
pub(crate) mod tests {

    use super::*;

    use mod_api::{ClientHost, FireworkExplosion, FireworkShape};

    use mod_api::FireworksRequest;

    use sunrise_client::particles::ParticleLevel;

    /// A client mod's fireworks burst either way, and blast and twinkle only when audible,
    /// as a rocket that isn't silent; a server draws nothing.
    #[test]
    #[ignore = "daily"]
    fn a_client_mods_fireworks_sound_only_when_audible() {
        use crate::game::server_side::native_entity_refs::tests::with_client_game;
        with_client_game(|game| {
            let eye = sunrise_core::geometry::Vec3::new(0.5, 200.0, 0.5);
            game.player = Some(sunrise_sim::motion::PlayerMotion::new(game.motion, eye).unwrap());
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
                game.client.particles.0.clear();
                game.client.particles.0.sounds.clear();
                let host: &mut dyn ClientHost = game;
                assert!(host.create_fireworks(&request(audible)));
                for _ in 0..20 {
                    let level = ParticleLevel {
                        world: &game.common.world,
                        facts: game.common.res.facts(),
                        collision: game.common.collision,
                        player: None,
                        cached_chunk: std::cell::Cell::new(None),
                    };
                    game.client.particles.0.tick(&level);
                }
                let heard: Vec<_> = game
                    .client
                    .particles
                    .0
                    .sounds
                    .iter()
                    .filter(|s| s.0.starts_with("minecraft:entity.firework_rocket."))
                    .map(|s| (s.0, s.1))
                    .collect();
                let expected: Vec<_> = expected.into_iter().map(|s| (s, at)).collect();
                assert_eq!(heard, expected, "audible {audible}");
                assert!(
                    !game.client.particles.0.is_empty(),
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
                sunrise_gameplay_common::game::execution_role::ExecutionRole::IntegratedServer;
            let host: &mut dyn ClientHost = game;
            assert!(
                !host.create_fireworks(&request(true)),
                "servers draw nothing"
            );
        });
    }
}
