#[allow(unused_imports)]
use super::block_entity_client_tick::ClientBlockEntityAccess;
#[allow(unused_imports)]
use super::block_entity_tick::ResidentBlockEntityAccess;
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use mod_api::{BlockEntityClientScope, BlockEntityScope, BlockEntitySideScope};

impl Game<'_> {
    /// Drain attachment notices before either logical side runs block entity tickers.
    pub(in crate::game) fn fire_pending_block_entity_loads(&mut self) {
        self.side_fire_pending_block_entity_loads();
    }
}

pub(in crate::game) fn authoritative_world_for_mods(
    resources: &crate::game::Resources,
    seed: i64,
) -> sunrise_client::world::World {
    let mut world = crate::game::world_for_mods(resources.facts().air, seed);
    world.authoritative_group_lifetime(resources.rest().behaviours.clone());
    world
}

/// Configure notice collection before this level can receive any block entities.
pub(in crate::game) fn world_for_mods(
    air: BlockStateId,
    seed: i64,
) -> sunrise_client::world::World {
    let mut world = sunrise_client::world::World::new(air, seed);
    world
        .block_entities
        .set_on_load_enabled(mod_api::__private::has_part_hook(
            mod_api::HookId::BlockEntityOnLoad,
        ));
    world
        .block_entities
        .set_data_load_enabled(mod_api::__private::has_part_hook(
            mod_api::HookId::BlockEntityLoadAdditional,
        ));
    world
}

/// Notify subscribers after this instance is attached and its saved data is applied.
pub(in crate::game) fn native_block_entity_on_load(
    call: mod_api::__private::BlockEntityOnLoadCall<'_>,
) {
    crate::game::native_hooks::fire::fire_block_entity_on_load(call);
}
