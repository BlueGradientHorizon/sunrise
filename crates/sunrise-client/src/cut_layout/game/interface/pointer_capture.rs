

#[cfg(test)]
pub(crate) mod tests {

#[test]
    #[ignore = "daily"]
    fn movement_key_held_behind_a_screen_counts_after_pointer_grab() {
        use super::*;
        use sunrise_assets::resource_source::{ResourceRoot, ResourceSource};

        let root = PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            Instant::now(),
            false,
            execution_role::ExecutionRole::Client,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = execution_role::ExecutionRole::Client;
        let mut game = sunrise_client_gameplay::game::ClientGame::new(
            &loaded.resources,
            loaded.client_res.unwrap(),
            loaded.pack,
            loaded.client_pack.unwrap(),
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&ResourceSource::directory(root)),
            None,
            sunrise_gameplay::game::server_side::integrated::serve,
            sunrise_gameplay::game::server_side::integrated::run_component_harness_on_server,
            crate::game::session_verifier(),
            crate::game::profile_resolver,
        )
        .unwrap();

        game.side.focused = true;
        game.side.screen = Screen::Paused;
        game.side.physical_keys.insert(KeyCode::KeyW);
        assert!(!game.movement_key_held(KeyCode::KeyW));

        game.side.screen = Screen::Playing;
        game.capture(true);
        assert_eq!(
            game.movement_key_held(KeyCode::KeyW),
            !cfg!(target_os = "macos")
        );
    }
}
