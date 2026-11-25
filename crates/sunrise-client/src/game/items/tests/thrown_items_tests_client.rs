#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::game_tests::tests::server_game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::item_entities::thrown_items_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{
    Audio, Options, Progress, Settings, load_resources,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::chunk::Chunk;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::entities::{ExperienceOrb, ItemEntity};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::ItemStack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play::{self, Clientbound as C};

/// The one sound `event` played since the last call, with its volume.
pub(in crate::game) fn sound(game: &mut crate::game::ClientGame<'_>, event: &str) -> (f32, f32) {
    let heard = game
        .side
        .audio
        .0
        .capture
        .replace(Vec::new())
        .unwrap_or_default();
    let sound = heard
        .iter()
        .find(|s| s.event == event)
        .unwrap_or_else(|| panic!("no {event} sound among {heard:?}"));
    (sound.volume, sound.pitch)
}

/// A window joined to a server through memory, with the sounds it plays
/// recorded: the client half of a pickup (`ClientPacketListener`).
pub(in crate::game) fn with_remote_game<R>(
    f: impl FnOnce(&mut crate::game::ClientGame<'_>) -> R,
) -> R {
    let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
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
        &mut |_, _| unreachable!(),
    )
    .unwrap();
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.clone();
    let mut game = crate::game::ClientGame::new(
        &loaded.resources,
        loaded.pack,
        loaded.gui.unwrap(),
        options,
        settings,
        Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
            root.to_path_buf(),
        )),
        None,
    )
    .unwrap();
    let (pipe, _server) = sunrise_protocol::connection::Pipe::pair();
    game.connect_local(pipe, "Steve").unwrap();
    game.side.audio.0.capture = Some(Vec::new());
    f(&mut game)
}
