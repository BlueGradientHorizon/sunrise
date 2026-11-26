//! `/time` addresses the clock the End runs on: the one `World.end_clock`
//! ticks and `world_clocks.dat` saves (`ServerClockManager.getInstance`).
use crate::game::{Dim, Options, Progress, Settings, execution_role, load_resources};

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
    options.execution_role = execution_role::ExecutionRole::IntegratedServer;
    let res = &*resources.insert(loaded.resources);
    let mut game = crate::game::ServerGame::new(
        res,
        loaded.pack,
        options,
        settings.sim_settings(),
        None,
        crate::game::profile_resolver,
    )
    .unwrap();
    game.build_registries();
    game.set_allow_commands(true);
    // The player stands in the End, so `/time` without `of` names its clock.
    game.dim = Dim::End;
    game
}
