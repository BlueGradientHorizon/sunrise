#[cfg(test)]
pub(crate) mod player_status_tests {
    use sunrise_gameplay_common::game::client_side::shader_packs::player_status_tests::rain_on_the_player;
    use sunrise_gameplay_common::game::client_side::shader_packs::player_status_tests::stand_player;
    use sunrise_level::effects::EffectInstance;
    use sunrise_gameplay_common::game_side;
    use sunrise_sim::motion::MotionWorld;

    /// The player booleans only custom uniforms read follow the window's player (spec 3.4):
    /// `LivingEntity.isAlive`, `Entity.isCurrentlyGlowing` (the Glowing effect fallback),
    /// `Entity.isInLava` and `Entity.isInWaterOrRain`.
    #[test]
    #[ignore = "daily"]
    fn pack_status_follows_the_player() {
        sunrise_client_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
            stand_player(game);

            let status = game.pack_player_status();
            assert!(status.alive);
            assert!(!status.glowing && !status.in_lava && !status.ridden && !status.wet);

            // Rain over the player's open plains column, and none once it stops.
            rain_on_the_player(game, true);
            assert!(game.pack_player_status().wet, "rain");
            rain_on_the_player(game, false);
            assert!(!game.pack_player_status().wet);
            game.player.as_mut().unwrap().fluids.was_touching_water = true;
            assert!(game.pack_player_status().wet, "water");
            game.player.as_mut().unwrap().fluids.was_touching_water = false;

            game.potions
                .effects
                .add(EffectInstance::new("glowing", 200, 0));
            assert!(game.pack_player_status().glowing);

            let lava = game
                .pack
                .assets
                .registry
                .default_state("minecraft:lava")
                .unwrap();
            assert!(game.world.set_stored(0, 64, 0, lava));
            let side = game_side!(game);
            let mut world = MotionWorld {
                side: &side,
                entities: &[],
                player: game.player.as_mut().unwrap(),
                scene: &mut game.world,
                assets: &game.pack.assets,
                collision_binding: game.collision,
                resources: game.motion,
                health: &game.health_resources,
                tag_items: None,
                loop_context: &game.loop_context,
                damage_hooks: &game.damage_hooks,
                block_registries: None,
            };
            world.fluid_base_tick().unwrap();
            assert!(game.pack_player_status().in_lava);

            game.player
                .as_mut()
                .unwrap()
                .set_health(0.0, false)
                .unwrap();
            assert!(!game.pack_player_status().alive);
        });
    }
}