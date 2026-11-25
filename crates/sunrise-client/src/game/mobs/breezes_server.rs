#[cfg(test)]
pub(in crate::game) mod tests {
    use crate::game::breezes::tests::*;
    use crate::game::entity::Entity;
    use sunrise_core::geometry::Vec3;

    /// `Entity.getOnPos(1.0E-5F)` keeps a fence's, a wall's or a fence gate's
    /// own cell while the feet rest on its 1.5-block collision; on a full block
    /// it is the block below the feet. Reading 0.2 below the feet, as the dust
    /// used to, lands in air on all three.
    #[test]
    #[ignore = "daily"]
    fn the_ground_of_a_breeze_is_the_fence_wall_or_gate_it_stands_on() {
        let mut resources = None;
        let mut game = game_with_breeze(&mut resources, Vec3::new(8.5, 65.5, 8.5));
        let registry = &game.pack.assets.registry;
        let stone = registry.default_state("minecraft:stone").unwrap();
        game.world.set(8, 63, 8, stone).unwrap();
        for name in [
            "minecraft:oak_fence",
            "minecraft:cobblestone_wall",
            "minecraft:oak_fence_gate",
        ] {
            let state = registry.default_state(name).unwrap();
            game.world.set(8, 64, 8, state).unwrap();
            let _ = game.mobs[0].update_retained_ground(
                &game.world,
                game.collision,
                &world_items!(game).entity_type_tags,
                true,
                None,
            );
            assert_eq!(
                game.breeze_ground(&game.mobs[0]),
                ([8, 64, 8], state),
                "{name}"
            );
        }
        // Replacing the shape between moves doesn't select a new support.
        game.world.set(8, 64, 8, stone).unwrap();
        assert_eq!(game.breeze_ground(&game.mobs[0]), ([8, 64, 8], stone));
        // The same breeze on the full block's top reads that block.
        game.mobs[0].position = Vec3::new(8.5, 65.0, 8.5);
        let _ = game.mobs[0].update_retained_ground(
            &game.world,
            game.collision,
            &world_items!(game).entity_type_tags,
            true,
            None,
        );
        assert_eq!(game.breeze_ground(&game.mobs[0]), ([8, 64, 8], stone));
    }
}
