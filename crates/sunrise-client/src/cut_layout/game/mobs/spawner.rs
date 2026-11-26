

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;

    /// `BaseSpawner.serverTick` with a player in range: a running delay only
    /// goes down by one a tick, drawing nothing from the level random and
    /// adding no mob. At 0 the spawn runs: with nothing to spawn it calls
    /// `delay`, which draws the next delay in [MinSpawnDelay, MaxSpawnDelay).
    #[test]
    #[ignore = "daily"]
    fn a_running_delay_counts_down_without_drawing() {
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            game.player = Some(
                sunrise_sim::motion::PlayerMotion::new(game.motion, Vec3::new(8.5, 64.0, 8.5))
                    .unwrap(),
            );
            let key = (8, 64, 12);
            let spawner = Spawner {
                spawn_delay: 3,
                ..Spawner::default()
            };
            game.world
                .block_entities
                .insert(key, BlockEntity::Spawner(Box::new(spawner)));
            let delay =
                |game: &sunrise_gameplay::game::ServerGame<'_>| match game.world.block_entities.get(&key) {
                    Some(BlockEntity::Spawner(s)) => s.spawn_delay,
                    _ => panic!("the spawner is gone"),
                };
            for expected in [2, 1, 0] {
                let mut before = game.world.random.clone();
                game.tick_spawner(key).unwrap();
                assert_eq!(delay(game), expected);
                assert_eq!(
                    game.world.random.next_long(),
                    before.next_long(),
                    "no draw at delay {expected}"
                );
                assert!(game.mobs.is_empty());
            }
            let mut before = game.world.random.clone();
            game.tick_spawner(key).unwrap();
            assert!((200..800).contains(&delay(game)), "{}", delay(game));
            assert_ne!(game.world.random.next_long(), before.next_long());
        });
    }
}