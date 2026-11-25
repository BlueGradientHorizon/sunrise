#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::{fixture_root, with_client_game, with_game_role};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{execution_role::ExecutionRole, lan::LanHost};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::prelude::ItemsHost;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{BuildCreativeModeTabContentsEvent, Hooks, HostSide, ModEntry};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Mutex;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::lan::{
    Lan, NetConfig,
    chunks::ChunkEncoder,
    data::SyncData,
    hub::{Hub, HubConfig},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::{Compound, Tag};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::loot::PackOverlay;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::Seen;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::Edit;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::stack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::stone_key;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::Observe;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::definition;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::replace_client_definition;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::book_level;

pub(in crate::game) fn install_dedicated_holders(
    game: &mut crate::game::ServerGame<'_>,
    max_level: i32,
) {
    let files = vec![(
        "enchantment/test:levels".into(),
        vec![definition(max_level).to_string().into_bytes()],
    )];
    let sync = Arc::new(
        SyncData::load(
            &game.options.resources,
            &PackOverlay::new(&files),
            game.res.item_defaults(),
            &sunrise_client::crafting::ResolvedSources::default(),
        )
        .unwrap(),
    );
    let net = NetConfig {
        listen: false,
        ..NetConfig::loopback(0, String::new(), 2)
    };
    let lan = Lan::start_with(sync.clone(), net).unwrap();
    let encoder = ChunkEncoder::new(
        [] as [(Arc<str>, bool); 0],
        &sync.builtin,
        &[],
        &[],
        Default::default(),
    );
    let config = HubConfig {
        default_game_mode: 0,
        guest_commands: false,
        force_game_mode: false,
        hardcore: false,
        view_distance: 2,
        simulation_distance: 2,
        dimension: game.dim.id().to_owned(),
        allow_flight: false,
        seed: 0,
        max_players: 2,
        integrated: false,
        levels: game.side.level_shapes,
        level_seeds: game.side.levels.0.runtime.seeds(),
    };
    game.lan = Some(LanHost::for_test(Hub::new(
        lan,
        sync,
        encoder,
        config,
        game.loop_context.clone(),
    )));
    game.native_lifecycle_open_started();
    game.native_lifecycle.world_open_ready();
}
