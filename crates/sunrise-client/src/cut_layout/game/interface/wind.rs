
#![cfg(test)]
use sunrise_core::geometry::Vec3;
use sunrise_sim::projectiles::Projectile;
use sunrise_sim::projectiles::ProjectileKind;
use sunrise_sim::projectiles::Hit;
#[cfg(test)]
#[path = "tests/wind_charged_tests.rs"]
pub(crate) mod wind_charged_tests;

#[cfg(test)]
#[path = "tests/wind_dragon_tests.rs"]
pub(crate) mod wind_dragon_tests;

#[cfg(test)]
pub(crate) mod tests {

use super::*;

#[test]
    #[ignore = "daily"]
    fn a_wind_push_waits_for_fresh_tnt_even_after_a_later_orb_ticked() {
        use std::sync::Arc;
        use sunrise_core::position::ChunkPos;
        use sunrise_level::chunk::Chunk;
        use sunrise_sim::join_seq::JoinSeq;

        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            let mut chunk = Chunk::empty(0, 0);
            chunk.set(4, 70, 8, stone.0);
            game.world.chunks.insert((0, 0), Arc::new(chunk));
            game.block_ticks
                .add_empty_chunk(ChunkPos::new(0, 0))
                .unwrap();
            assert!(game.run_command("forceload add 0 0"));
            game.server.tickets.0.update(None);

            game.server.ordered.announced = JoinSeq::upcoming();
            assert!(
                game.run_command(
                    "summon tnt 8.25 70.5 8.5 {fuse:80,Motion:[0d,0d,0d],NoGravity:1b}"
                )
            );
            assert!(game.run_command("summon experience_orb 12.5 70.5 12.5 {Value:1}"));
            let at = Vec3::new(5.0, 70.5, 8.5);
            game.add_projectile(Projectile::new(ProjectileKind::BreezeWindCharge, at, None));
            let charge = game.creatures.projectiles[0].clone();
            game.begin_ordered_ticks();
            game.run_ordered_ticks(charge.join_seq(), false).unwrap();
            assert_eq!(game.server.ordered.done, Some(game.orbs[0].join_seq()));
            assert_eq!(game.orbs[0].age, 1);
            assert_eq!(game.tnts[0].fuse, 80);

            game.wind_charge_hit(
                &charge,
                Hit::Block {
                    pos: [4, 70, 8],
                    at,
                    face: [1, 0, 0],
                },
            )
            .unwrap();
            game.run_ordered_ticks(JoinSeq::MAX, true).unwrap();
            assert_eq!(game.tnts[0].fuse, 79);
            assert_eq!(game.tnts[0].position.x, 8.25);
            game.apply_late_wind_pushes();
            // Three blocks from the centre, unobstructed: (1 - 3 / 6) due east.
            assert_eq!(game.tnts[0].velocity.x, 0.5);

            game.server.ordered.announced = JoinSeq::upcoming();
            game.begin_ordered_ticks();
            game.run_ordered_ticks(charge.join_seq(), false).unwrap();
            assert_eq!(game.tnts[0].fuse, 78);
            assert_eq!(game.tnts[0].position.x, 8.75);
        });
    }
}
