//! A projectile's block effects through its real tick (`Projectile.tick`:
//! move, `applyEffectsFromBlocks`, `baseTick`, then its hit).
use super::*;
use crate::game::native_entity_refs::tests::{run_fresh_cases, with_game};
use std::sync::Arc;
use sunrise_client::chunk::Chunk;

const CASE: &str = "SUNRISE_PROJECTILE_BLOCK_EFFECTS_CASE";

/// One projectile, still, inside `block` at the origin, ticked once; the
/// sounds it made.
fn tick_inside(game: &mut Game<'_>, block: &str, mut shot: Projectile) -> Vec<String> {
    game.options.dedicated = Some(Vec::new());
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    let state = game.pack.assets.registry.default_state(block).unwrap();
    game.world.set(0, 64, 0, state).unwrap();
    shot.id = 71;
    shot.velocity = Vec3::ZERO;
    game.creatures.projectiles.push(shot);
    game.audio.capture = Some(Vec::new());
    game.tick_projectiles().unwrap();
    let sounds = game.audio.capture.take().unwrap();
    sounds.into_iter().map(|sound| sound.event).collect()
}

fn at() -> Vec3 {
    Vec3::new(0.5, 64.5, 0.5)
}

#[test]
#[ignore = "daily"]
fn fire_lava_and_water_reach_a_projectile_in_its_tick() {
    if run_fresh_cases(
        "game::creatures::projectile_block_effects_tests::fire_lava_and_water_reach_a_projectile_in_its_tick",
        CASE,
        &["fire", "lava", "water"],
    ) {
        return;
    }
    with_game(|game| match std::env::var(CASE).as_deref() {
        // FireBlock.entityInside lights it for 160 ticks (`igniteForTicks`
        // on a count of 0), and the base tick after the walk takes one.
        Ok("fire") => {
            let shot = Projectile::new(ProjectileKind::Snowball, at(), None);
            tick_inside(game, "minecraft:fire", shot);
            assert_eq!(game.creatures.projectiles[0].fire_ticks, 159);
        }
        // Entity.lavaHurt on a shulker bullet: its hurtServer destroys it,
        // with its hurt sound, and the burn plays as hurtServer said true.
        Ok("lava") => {
            let shot = Projectile::new(
                ProjectileKind::ShulkerBullet {
                    target: None,
                    dir: None,
                    steps: 0,
                    delta: [0.0; 3],
                    seed: 0,
                },
                at(),
                None,
            );
            let sounds = tick_inside(game, "minecraft:lava", shot);
            assert!(game.creatures.projectiles.is_empty(), "the bullet is gone");
            assert!(
                sounds
                    .iter()
                    .any(|s| s == "minecraft:entity.shulker_bullet.hurt")
            );
            assert!(sounds.iter().any(|s| s == "minecraft:entity.generic.burn"));
        }
        // Water puts a burning projectile out, and the move that did plays
        // the extinguish hiss.
        Ok("water") => {
            let mut shot = Projectile::new(ProjectileKind::Snowball, at(), None);
            shot.fire_ticks = 100;
            let sounds = tick_inside(game, "minecraft:water", shot);
            assert_eq!(game.creatures.projectiles[0].fire_ticks, 0);
            assert!(
                sounds
                    .iter()
                    .any(|s| s == "minecraft:entity.generic.extinguish_fire")
            );
        }
        case => panic!("unknown case {case:?}"),
    });
}
