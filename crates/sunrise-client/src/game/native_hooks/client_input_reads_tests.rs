#[cfg(test)]
mod tests {
    use crate::game::native_entity_refs::tests::with_game;
    use sunrise_client::motion::PlayerMotion;
    use sunrise_core::geometry::Vec3;
    use winit::{event::MouseButton, keyboard::KeyCode};

    #[test]
    #[ignore = "daily"]
    fn mapping_physical_down_follows_the_binding_and_ignores_the_logical_state() {
        with_game(|game| {
            let forward_mapping = game.vanilla_key_mapping("key.forward").unwrap();
            let attack_mapping = game.vanilla_key_mapping("key.attack").unwrap();
            game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
            // Only the mappings `mapping` answers for: a mod's own mapping is read with `is_down`.
            assert_eq!(game.mapping_physical_down("key.not_a_mapping"), Ok(None));
            assert!(game.mapping("key.not_a_mapping").unwrap().is_none());
            assert_eq!(forward_mapping.physical_down(game), Ok(false));
            game.physical_keys.insert(KeyCode::KeyW);
            assert_eq!(forward_mapping.physical_down(game), Ok(true));
            // A write to the logical mapping does not move the physical read.
            game.set_mapping_down("key.forward", false).unwrap();
            assert!(!forward_mapping.is_down(game).unwrap());
            assert_eq!(forward_mapping.physical_down(game), Ok(true));
            // Rebound, it reads the new key, and the old one no longer counts.
            game.settings.keys.set("key.forward", "key.keyboard.up");
            assert_eq!(forward_mapping.physical_down(game), Ok(false));
            game.physical_keys.insert(KeyCode::ArrowUp);
            assert_eq!(forward_mapping.physical_down(game), Ok(true));
            // A mapping on a mouse button reads the button, by name too.
            assert_eq!(attack_mapping.physical_down(game), Ok(false));
            game.physical_buttons.insert(MouseButton::Left);
            assert_eq!(attack_mapping.physical_down(game), Ok(true));
            assert!(game.is_key_down("key.mouse.left"));
            assert!(!game.is_key_down("key.mouse.right"));
            game.physical_buttons.clear();
            assert!(!game.is_key_down("key.mouse.left"));
            // A mapping bound to no key is never down.
            game.settings.keys.set("key.attack", "key.keyboard.unknown");
            game.physical_buttons.insert(MouseButton::Left);
            assert_eq!(attack_mapping.physical_down(game), Ok(false));
        });
    }
}
