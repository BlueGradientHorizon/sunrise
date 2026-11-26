#![cfg(test)]
#[cfg(test)]
mod tests {
    use mod_api::{LevelRef, WorldEpoch};
    use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1;

    #[test]
    #[ignore = "daily"]
    fn border_writes_move_the_shared_border_and_refuse_bad_values() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let level = game.level(game.dim.id()).unwrap();
            let stale = LevelRef::new(WorldEpoch::new(level.epoch().get() + 1), level.dimension());

            assert!(level.set_world_border_center(game, [100.0, -50.0]));
            assert_eq!(game.border.center(), [100.0, -50.0]);
            assert!(!level.set_world_border_center(game, [f64::NAN, 0.0]));
            assert!(
                !level.set_world_border_center(game, [0.0, 3.0e7]),
                "past the limit"
            );
            assert!(!stale.set_world_border_center(game, [0.0, 0.0]));
            assert_eq!(
                game.border.center(),
                [100.0, -50.0],
                "refusals change nothing"
            );

            assert!(level.set_world_border_size(game, 200.0, 0));
            assert_eq!(level.world_border(game), Some([0.0, -150.0, 200.0, 50.0]));
            assert!(level.set_world_border_size(game, 100.0, 20));
            assert_eq!(game.border.remaining_ticks(), 20, "a timed size lerps");
            assert_eq!(game.border.target_size(), 100.0);
            assert!(!level.set_world_border_size(game, 0.5, 0));
            assert!(!level.set_world_border_size(game, 100.0, -1));

            assert!(level.set_world_border_damage(game, 0.5, 3.0));
            assert_eq!(
                (game.border.damage_per_block, game.border.safe_zone),
                (0.5, 3.0)
            );
            assert!(!level.set_world_border_damage(game, -1.0, 3.0));
            assert!(level.set_world_border_warning(game, 8, 100));
            assert_eq!(
                (game.border.warning_distance, game.border.warning_time),
                (8, 100)
            );
            assert!(!level.set_world_border_warning(game, -1, 100));
            assert!(!stale.set_world_border_warning(game, 1, 1));
        });
    }
}
use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
