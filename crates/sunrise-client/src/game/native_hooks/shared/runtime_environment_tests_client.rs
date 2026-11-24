#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::runtime_environment_tests::*;

#[test]
#[ignore = "daily"]
fn client_host_reads_playing_and_key_mappings() {
    use super::super::Screen;
    use super::super::native_entity_refs::tests::with_game;
    use crate::game::execution_role::ExecutionRole;
    use mod_api::prelude::ClientInputClientHost;
    use mod_api::{MappingError, Text};
    use winit::keyboard::KeyCode;

    with_game(|game| {
        game.options.execution_role = ExecutionRole::Client;
        game.client.screen = Screen::Playing;
        assert!(mod_api::prelude::ClientInputClientHost::playing(game));
        game.client.screen = Screen::Chat;
        assert!(!mod_api::prelude::ClientInputClientHost::playing(game));

        // Vanilla's default bindings: forward is W, attack the left mouse button.
        let forward = ClientInputClientHost::vanilla_key_mapping(game, "key.forward").unwrap();
        assert_eq!(forward.is_down(game), Ok(false));
        assert_eq!(forward.translated_key_message(game), Ok(Text::literal("W")));
        game.insert_device_key(KeyCode::KeyW);
        assert_eq!(forward.is_down(game), Ok(true));
        let attack = ClientInputClientHost::vanilla_key_mapping(game, "key.attack").unwrap();
        assert_eq!(
            attack.translated_key_message(game),
            Ok(Text::translate("key.mouse.left", Vec::new()))
        );
        assert_eq!(
            ClientInputClientHost::vanilla_key_mapping(game, "key.mymod.boost"),
            Err(MappingError::Unknown {
                key: "key.mymod.boost".into(),
            }),
        );

        game.options.execution_role = ExecutionRole::IntegratedServer;
        assert_eq!(
            ClientInputClientHost::vanilla_key_mapping(game, "key.forward"),
            Err(MappingError::Unavailable),
        );
        assert_eq!(forward.binding(game), Err(MappingError::Unavailable));
    });
}
