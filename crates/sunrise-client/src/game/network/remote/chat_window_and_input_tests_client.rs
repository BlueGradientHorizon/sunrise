#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tests::chat_window_and_input_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{
    Audio, Game, Options, Progress, Screen, Settings, execution_role::ExecutionRole,
    load_resources, native_entity_refs::tests::with_game,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::connection::{Connection, Pipe};

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
        crate::game::execution_role::ExecutionRole::IntegratedServer,
        &Progress::new(),
        &mut |_, _| unreachable!("no menu handoff"),
    )
    .unwrap();
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.clone();
    options.execution_role = ExecutionRole::Client;
    let mut game = Game::new(
        &loaded.resources,
        loaded.pack,
        loaded.gui.unwrap(),
        options,
        settings,
        Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
            root.clone(),
        )),
        None,
    )
    .unwrap();

    // No player: the title screen.
    game.player = None;
    assert!(game.open_chat("/tp ").is_err());
    assert!(game.ui.chat_screen.is_none());
    game.player = Some(
        sunrise_client::motion::PlayerMotion::new(game.motion, sunrise_core::geometry::Vec3::ZERO)
            .unwrap(),
    );
    game.client.screen = Screen::Paused;
    assert!(game.open_chat("/tp ").is_err());
    assert!(game.ui.chat_screen.is_none());
    game.client.screen = Screen::Playing;
    assert!(game.open_chat("/tp ").is_ok());
    assert!(game.client.screen == Screen::Chat);
    assert_eq!(game.ui.chat_screen.as_ref().unwrap().input.value, "/tp ");
    // The chat is a screen too: a second call doesn't replace what's typed.
    assert!(game.open_chat("hello").is_err());
    assert_eq!(game.ui.chat_screen.as_ref().unwrap().input.value, "/tp ");
}
