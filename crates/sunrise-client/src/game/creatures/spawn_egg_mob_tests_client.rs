#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::spawn_egg_mob_tests::posted;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::spawn_egg_mob_tests::spawn;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::spawn_egg_mob_tests::target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::spawn_egg_mob_tests::watch;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::spawn_egg_mob_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{
    Audio, Game, InteractTarget, Options, Progress, Screen, Settings, execution_role,
    load_resources,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::chunk::Chunk;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::ItemStack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::{Mob, Species, Who};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::motion::PlayerMotion;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::ChunkPos;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::{LegacyRandom, RandomSource};

pub(in crate::game) fn game<'a>(
    resources: &'a mut Option<std::sync::Arc<crate::game::Resources>>,
) -> Game<'a> {
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
        &mut |_, _| unreachable!(),
    )
    .unwrap();
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.clone();
    options.execution_role = execution_role::ExecutionRole::IntegratedServer;
    let res = &*resources.insert(loaded.resources);
    let mut game = Game::new(
        res,
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
    game.build_registries();
    game.set_allow_commands(true);
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    game.block_ticks
        .add_empty_chunk(ChunkPos::new(0, 0))
        .unwrap();
    game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
    game.client.screen = Screen::Playing;
    game.selected = 0;
    game.set_mode_id(0);
    game
}
