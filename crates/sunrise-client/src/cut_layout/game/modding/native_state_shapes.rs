

#![cfg(test)]
#[cfg(test)]
pub(crate) mod tests {
    use {crate::game::native_state_shapes::*, sunrise_gameplay_common::game::native_state_shapes::{BlockShapeKind, Outline}};
    use sunrise_gameplay_common::game::api_values::TryIntoApi;

    #[test]
    #[ignore = "daily"]
    fn supplied_shape_state_does_not_replace_world_state() {
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            let level = game.level("minecraft:overworld").unwrap();
            let pos = [0, 64, 0];
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            let slab = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone_slab")
                .unwrap();
            let before = game.world.block(pos[0], pos[1], pos[2]);
            assert_ne!(before, stone);
            for kind in [BlockShapeKind::Outline, BlockShapeKind::Collision] {
                assert_eq!(
                    level.block_shape(game, stone.try_into_api().unwrap(), pos, kind),
                    Some(vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]])
                );
                assert_eq!(
                    level.block_shape(game, slab.try_into_api().unwrap(), pos, kind),
                    Some(vec![[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]])
                );
                assert_eq!(
                    level.block_shape(game, before.try_into_api().unwrap(), pos, kind),
                    Some(Vec::new())
                );
            }
            assert_eq!(game.world.block(pos[0], pos[1], pos[2]), before);
        });
    }
}
use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
