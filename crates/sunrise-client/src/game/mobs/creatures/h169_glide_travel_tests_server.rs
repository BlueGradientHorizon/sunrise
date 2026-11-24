#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::h169_glide_travel_tests::with_game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::h169_glide_travel_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{
    Audio, Options, Progress, Settings, load_resources,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{chunk::Chunk, motion::PlayerMotion};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::Vec3, position::ChunkPos, random::RandomSource,
};

pub(in crate::game) fn with_role_game(
    role: crate::game::execution_role::ExecutionRole,
    test: impl FnOnce(&mut Game<'_>),
) {
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
    options.game_dir = Some(
        std::path::PathBuf::from(std::env::var_os("SUNRISE_HOME").unwrap())
            .join(format!("h169-{:?}", std::thread::current().id())),
    );
    options.execution_role = role;
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
    game.build_registries();
    game.set_allow_commands(true);

    let mut center = Chunk::empty(0, 0);
    center.lit = true;
    game.world.chunks.insert((0, 0), Arc::new(center));
    game.block_ticks
        .add_empty_chunk(ChunkPos::new(0, 0))
        .unwrap();
    assert!(game.run_command("difficulty normal"));
    assert!(game.run_command("forceload add 0 0"));
    for x in -1..=1 {
        for z in -1..=1 {
            if x == 0 && z == 0 {
                continue;
            }
            let mut chunk = Chunk::empty(x, z);
            chunk.lit = true;
            game.world.chunks.insert((x, z), Arc::new(chunk));
            game.block_ticks
                .add_empty_chunk(ChunkPos::new(x, z))
                .unwrap();
        }
    }
    game.load_ticking_neighbourhood(0, 0);
    game.player = Some(PlayerMotion::new(game.motion, Vec3::new(2.5, 200.0, 2.5)).unwrap());
    game.creative = true;
    assert!(game.run_command("fill 0 199 0 31 199 31 stone"));
    game.world.random.set_seed(169);
    game.update_chunk_tickets();
    assert!(crate::game::ticking_chunk(&game.world, 0, 0));
    assert!(game.tickets.0.levels().block_ticking(0, 0));
    test(&mut game);
}
