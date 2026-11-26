//! A client mod's `raycast` and `view_hit` hit loaded blocks only, far terrain on or off.
use crate::game::{
    execution_role,
    launch_options::Options,
    resource_load::{Progress, load_resources},
};
use std::sync::Arc;
use sunrise_client::{audio::Audio, settings::Settings};
use sunrise_core::geometry::Vec3;
use sunrise_level::chunk::Chunk;
use sunrise_sim::motion::PlayerMotion;

fn game<'a>(resources: &'a mut Option<Arc<crate::game::Resources>>) -> crate::game::ClientGame<'a> {
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
        execution_role::ExecutionRole::Client,
        &Progress::new(),
        &mut |_, _| unreachable!(),
    )
    .unwrap();
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.clone();
    options.execution_role = execution_role::ExecutionRole::Client;
    let res = &*resources.insert(loaded.resources);
    let mut game = crate::game::ClientGame::new(
        res,
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
    )
    .unwrap();
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    let stone = game
        .pack
        .assets
        .registry
        .default_state("minecraft:stone")
        .unwrap();
    for x in 0..=15 {
        for z in 0..=15 {
            game.world.set(x, 99, z, stone).unwrap();
        }
    }
    game
}

#[test]
#[ignore = "daily"]
fn mod_raycasts_hit_loaded_blocks_only_with_far_terrain_on_or_off() {
    let mut resources = None;
    let mut game = game(&mut resources);
    // Looking straight down at the stone under the player.
    game.pitch = 90.0;
    let at = |game: &mut crate::game::ClientGame<'_>, x: f64| {
        game.player = Some(PlayerMotion::new(game.motion, Vec3::new(x, 100.0, 0.5)).unwrap());
    };
    at(&mut game, 0.5);
    let without = game.view_hit().expect("the stone below");
    assert_eq!(
        (without.1.block_x, without.1.block_y, without.1.block_z),
        (0, 99, 0)
    );
    assert_eq!(game.raycast(8.0), Some(without.1.clone()));
    assert_eq!(game.raycast(1.0), None, "past the distance asked for");
    game.side.settings.sunrise.lod_distance = 256;
    game.side.lod.0 = Some(sunrise_client::lod::Lod::new(
        game.side.streaming.lod_source(),
        game.worker_context(),
    ));
    assert_eq!(
        game.view_hit(),
        Some(without.clone()),
        "far terrain changes nothing"
    );
    assert_eq!(game.raycast(8.0), Some(without.1));
    // Over a chunk that isn't loaded, far terrain or not, there is nothing to hit.
    at(&mut game, 40.5);
    assert_eq!(game.view_hit(), None);
    assert_eq!(game.raycast(1024.0), None);
}
