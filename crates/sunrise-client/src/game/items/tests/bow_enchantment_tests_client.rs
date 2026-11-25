#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::use_hand::bow_enchantment_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{
    Audio, Game, Options, Progress, Settings, load_resources,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::ItemStack;

/// A game on the prepared resources with a creative player (as `crossbow.rs`).
pub(in crate::game) fn game_with_player<'a>(
    resources: &'a mut Option<std::sync::Arc<crate::game::Resources>>,
    root: &std::path::Path,
) -> crate::game::ServerGame<'a> {
    let settings = Settings::default();
    let loaded = load_resources(
        sunrise_assets::resource_source::ResourceRoot::directory(root.to_path_buf()),
        Vec::new(),
        None,
        root,
        &settings,
        std::time::Instant::now(),
        false,
        crate::game::execution_role::ExecutionRole::IntegratedServer,
        &Progress::new(),
        &mut |_, _| unreachable!(),
    )
    .unwrap();
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.to_path_buf();
    options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
    let res = &*resources.insert(loaded.resources);
    let mut game = crate::game::ServerGame::new(
        res,
        loaded.pack,
        loaded.gui.unwrap(),
        options,
        settings,
        None,
    )
    .unwrap();
    game.creative = true;
    game.player = Some(
        sunrise_client::motion::PlayerMotion::new(
            game.motion,
            sunrise_core::geometry::Vec3::new(10.5, 64.0, -3.25),
        )
        .unwrap(),
    );
    // A still player: `known_player_movement` is the last tick's step.
    game.previous_position = game.player.as_ref().expect("a player").position;
    game.start_audio_capture();
    game
}
