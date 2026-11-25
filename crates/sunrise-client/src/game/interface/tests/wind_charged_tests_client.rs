#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::Target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::wind_charged_tests::BREEZE_BURST;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::wind_charged_tests::WIND_CHARGE_BURST;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::wind_charged_tests::a_wind_charged_zombie_dies;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::wind_charged_tests::boat;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::wind_charged_tests::boat_speed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::wind_charged_tests::with_game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::wind_charged_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{
    Audio, Game, Options, Progress, Settings, load_resources,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::chunk::Chunk;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::food::Difficulty;

/// `AbstractWindCharge.explode` hands `ServerLevel.explode` the charge itself
/// as the source, so `playSound` is `!charge.isSilent()`: a `Silent` wind
/// charge's burst is mute, its gust and its push stay. The sound is the wind
/// charge's own, as `WindCharge.explode` passes it.
#[test]
#[ignore = "daily"]
fn a_silent_wind_charge_bursts_without_a_sound() {
    let Some(outcome) = with_game(|game| {
        let mut outcome = Vec::new();
        for silent in [false, true] {
            let boat = boat(game);
            let (charge, breeze) = (heard(game, WIND_CHARGE_BURST), heard(game, BREEZE_BURST));
            game.particles.0.requested = Some(Vec::new());
            let nbt = if silent {
                "{Silent:1b,Motion:[0.0,-1.0,0.0]}"
            } else {
                "{Motion:[0.0,-1.0,0.0]}"
            };
            game.summon_with_nbt("minecraft:wind_charge", [4.5, 66.0, 4.5], nbt)
                .unwrap()
                .expect("summoned wind charge");
            for _ in 0..40 {
                game.tick_creatures().unwrap();
            }
            let gust = game.particles.0.requested.take().unwrap_or_default();
            outcome.push((
                silent,
                heard(game, WIND_CHARGE_BURST) - charge,
                heard(game, BREEZE_BURST) - breeze,
                gust.iter()
                    .filter(|(p, _, _)| {
                        p.ty == sunrise_client::particles::ParticleType::GustEmitterSmall
                    })
                    .count(),
                boat_speed(game, boat) > 0.0,
            ));
            game.boats.clear();
        }
        outcome
    }) else {
        return;
    };
    assert_eq!(
        outcome,
        vec![
            // Heard once, as the wind charge's, with the gust and the boat thrown.
            (false, 1, 0, 1, true),
            // A silent charge's burst is mute, and still pushes and draws.
            (true, 0, 0, 1, true),
        ]
    );
}

/// `WindChargedMobEffect.onMobRemoved` calls `ServerLevel.explode` with
/// `SoundEvents.BREEZE_WIND_CHARGE_BURST`, so the burst is heard as the
/// breeze's, not as the wind charge's; `ServerLevel.explode` sends
/// `playSound = source == null || !source.isSilent()`, so a `Silent` mob's
/// burst is mute. The particle and the knockback stay either way.
#[test]
#[ignore = "daily"]
fn a_wind_charged_mob_bursts_with_the_breezes_sound_unless_it_is_silent() {
    let Some(outcome) = with_game(|game| {
        let mut outcome = Vec::new();
        for silent in [false, true] {
            let boat = boat(game);
            let (breeze, charge) = (heard(game, BREEZE_BURST), heard(game, WIND_CHARGE_BURST));
            game.particles.0.requested = Some(Vec::new());
            a_wind_charged_zombie_dies(game, silent);
            let gust = game.particles.0.requested.take().unwrap_or_default();
            outcome.push((
                silent,
                heard(game, BREEZE_BURST) - breeze,
                heard(game, WIND_CHARGE_BURST) - charge,
                gust.iter()
                    .filter(|(p, _, _)| {
                        p.ty == sunrise_client::particles::ParticleType::GustEmitterLarge
                    })
                    .count(),
                boat_speed(game, boat) > 0.0,
            ));
            game.boats.clear();
        }
        outcome
    }) else {
        return;
    };
    assert_eq!(
        outcome,
        vec![
            // Heard once, as the breeze's, with the gust and the boat thrown.
            (false, 1, 0, 1, true),
            // A silent mob's burst is mute, and still pushes and draws.
            (true, 0, 0, 1, true),
        ]
    );
}

/// How many times `sound` was played as a positional sound.
pub(in crate::game) fn heard(game: &crate::game::ClientGame<'_>, sound: &str) -> u32 {
    game.side.audio.0.heard.get(sound).copied().unwrap_or(0)
}
