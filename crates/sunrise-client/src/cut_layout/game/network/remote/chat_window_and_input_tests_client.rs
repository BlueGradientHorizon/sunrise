use {sunrise_gameplay_common::game::Audio, sunrise_gameplay_common::game::Options, sunrise_gameplay_common::game::Progress, sunrise_gameplay_common::game::Screen, sunrise_gameplay_common::game::Settings, sunrise_gameplay_common::game::execution_role::ExecutionRole, sunrise_gameplay_common::game::load_resources};

/// A mod's pre-filled chat opens only over the world, with its text
/// typed in and nothing sent.
#[test]
#[ignore = "daily"]
fn mod_open_chat_fills_the_box_only_over_the_world() {
    let root = std::path::PathBuf::from(
        std::env::var_os("SUNRISE_TEST_RESOURCES")
            .expect("set SUNRISE_TEST_RESOURCES to prepared resources"),
    );
    let settings = Settings::default();
    let loaded = load_resources(
        sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
        Vec::new(),
        None,
        &root,
        &settings,
        std::time::Instant::now(),
        false,
        sunrise_gameplay_common::game::execution_role::ExecutionRole::Client,
        &Progress::new(),
        &mut |_, _| unreachable!("no menu handoff"),
    )
    .unwrap();
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.clone();
    options.execution_role = ExecutionRole::Client;
    let mut game = sunrise_client_gameplay::game::ClientGame::new(
        &loaded.resources,
        loaded.client_res.unwrap(),
        loaded.pack,
        loaded.client_pack.unwrap(),
        loaded.gui.unwrap(),
        options,
        settings,
        Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
            root.clone(),
        )),
        None,
        sunrise_gameplay::game::server_side::integrated::serve,
        sunrise_gameplay::game::server_side::integrated::run_component_harness_on_server,
        crate::game::session_verifier(),
        crate::game::profile_resolver,
    )
    .unwrap();

    // No player: the title screen.
    game.player = None;
    assert!(game.open_chat("/tp ").is_err());
    assert!(game.side.chat_ui.chat_screen.is_none());
    game.player = Some(
        sunrise_sim::motion::PlayerMotion::new(game.motion, sunrise_core::geometry::Vec3::ZERO)
            .unwrap(),
    );
    game.side.screen = Screen::Paused;
    assert!(game.open_chat("/tp ").is_err());
    assert!(game.side.chat_ui.chat_screen.is_none());
    game.side.screen = Screen::Playing;
    assert!(game.open_chat("/tp ").is_ok());
    assert!(game.side.screen == Screen::Chat);
    assert_eq!(
        game.side.chat_ui.chat_screen.as_ref().unwrap().input.value,
        "/tp "
    );
    // The chat is a screen too: a second call doesn't replace what's typed.
    assert!(game.open_chat("hello").is_err());
    assert_eq!(
        game.side.chat_ui.chat_screen.as_ref().unwrap().input.value,
        "/tp "
    );
}
