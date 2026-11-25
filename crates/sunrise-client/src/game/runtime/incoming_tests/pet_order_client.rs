#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::core_tests::pet_order_tests::WOLF;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::core_tests::pet_order_tests::use_on_wolf;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::core_tests::pet_order_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::chunk::Chunk;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::Mob;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::ChunkPos;

pub(in crate::game) fn game<'a>(
    resources: &'a mut Option<std::sync::Arc<crate::game::Resources>>,
) -> crate::game::ServerGame<'a> {
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
    options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
    let res = resources.insert(loaded.resources);
    let mut game = crate::game::ServerGame::new(
        res,
        loaded.pack,
        loaded.gui.unwrap(),
        options,
        settings,
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
    assert!(game.run_command("fill 0 63 0 15 63 15 stone"));
    game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
    game.selected = 0;
    game.set_mode_id(0);
    let kind = game.res.mob_types().get("minecraft:wolf").unwrap().clone();
    let mut wolf = Mob::new(WOLF, kind, Vec3::new(0.5, 64.0, 2.0), 0.0);
    sunrise_client::mobs::tame::set_tame(&mut wolf, true);
    wolf.extra.owner = Some(game.acting_player_uuid());
    wolf.ground.on_ground = true;
    game.mobs.push(wolf);
    game
}
