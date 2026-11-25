use crate::game::commands::world_cmds::particle_command_tests::game;
use mod_api::client::particles::PositionSource;
use std::sync::Arc;
use sunrise_client::{
    chunk::Chunk,
    particles::{ParticleLevel, ParticleOptions},
};

/// The crumbs the command names are queued on the server and drawn on the client:
/// `TerrainParticle.CrumblingProvider` keeps no particle for options that carry no block.
#[test]
#[ignore = "daily"]
fn block_crumble_draws_the_block_it_names() {
    let mut resources = None;
    let mut server = game(&mut resources);
    let options = drawn(
        &mut server,
        "particle minecraft:block_crumble{block_state:\"minecraft:stone\"} 0.5 100.5 0.5 0 0 0 0 1",
    );
    crate::game::native_entity_refs::tests::with_client_game(|client| {
        client
            .world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        client.side.particles.0.created_requests = None;
        client.side.particles.0.clear();
        client
            .side
            .particles
            .0
            .create(&options, [0.5, 100.5, 0.5], [0.0; 3]);
        let level = ParticleLevel {
            world: &client.world,
            facts: client.res.facts(),
            collision: client.collision,
            player: None,
            cached_chunk: std::cell::Cell::new(None),
        };
        client.side.particles.0.tick(&level);
        assert_eq!(client.side.particles.0.len(), 1, "the crumble is drawn");
    });
}

/// The one particle the server queues for `command`, decoded into client `ParticleOptions`.
pub(in crate::game) fn drawn(
    game: &mut crate::game::ServerGame<'_>,
    command: &str,
) -> ParticleOptions {
    use crate::lan::server_effects::ServerParticleData;
    game.side.pending_particles.clear();
    assert!(game.run_command(command), "{command}");
    let mut made = std::mem::take(&mut game.side.pending_particles);
    assert_eq!(made.len(), 1, "{command}");
    let queued = made.remove(0);
    if let ServerParticleData::Mod { name, options } = &queued.particle.data {
        return crate::game::lan_particles::decode_mod_particle(name, options)
            .expect("mod particle decodes");
    }
    let ty = queued.particle.ty;
    match queued.particle.data {
        ServerParticleData::None => ParticleOptions::simple(ty),
        ServerParticleData::Block(state) => {
            let block = [
                queued.at[0].floor() as i32,
                queued.at[1].floor() as i32,
                queued.at[2].floor() as i32,
            ];
            let source = game.res.facts().get(state).name.clone();
            let (sprite, tint) = game.particle_sprite(state, block).unwrap_or_else(|| {
                (
                    format!(
                        "block/{}",
                        source.strip_prefix("minecraft:").unwrap_or(&source)
                    ),
                    0xFF_FFFF,
                )
            });
            ParticleOptions::block(ty, sprite, tint).with_source(&source)
        }
        ServerParticleData::ColorScale { rgb, scale } => ParticleOptions {
            color: [rgb[0], rgb[1], rgb[2], 1.0],
            scale,
            ..ParticleOptions::simple(ty)
        },
        ServerParticleData::ColorScale2 { from, to, scale } => ParticleOptions {
            color: [from[0], from[1], from[2], 1.0],
            to_color: to,
            scale,
            ..ParticleOptions::simple(ty)
        },
        ServerParticleData::ColorPower { rgb, power } => ParticleOptions {
            color: [rgb[0], rgb[1], rgb[2], 1.0],
            scale: power,
            ..ParticleOptions::simple(ty)
        },
        ServerParticleData::ColorRgba(rgba) => ParticleOptions {
            color: rgba,
            ..ParticleOptions::simple(ty)
        },
        ServerParticleData::Scalar(v) => ParticleOptions {
            scale: v,
            ..ParticleOptions::simple(ty)
        },
        ServerParticleData::Delay(ticks) => ParticleOptions {
            scale: ticks as f32,
            ..ParticleOptions::simple(ty)
        },
        ServerParticleData::Vibration { source, ticks } => {
            let to = match &source {
                PositionSource::Block(p) => [
                    f64::from(p[0]) + 0.5,
                    f64::from(p[1]) + 0.5,
                    f64::from(p[2]) + 0.5,
                ],
                _ => queued.at,
            };
            ParticleOptions {
                vibration: Some((source, to, ticks)),
                ..ParticleOptions::simple(ty)
            }
        }
        other => panic!("unexpected particle data for {command}: {other:?}"),
    }
}
