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
        self.fire_pending_block_entity_data_loads();
        if !self.world.block_entities.has_pending_on_load() {
            return;
        }
        let server = self.is_logical_server();
        let level = if server {
            self.level(self.dim.id())
        } else {
            self.native_client_level()
        };
        let Some(level) = level else { return };
        let mut remaining = self.world.block_entities.pending_on_load_len();
        while let Some((pos, generation)) = self
            .world
            .block_entities
            .next_on_load_in_batch(&mut remaining)
        {
            if !mod_api::__private::has_part_hook(mod_api::HookId::BlockEntityOnLoad) {
                continue;
            }
            let Some(BlockEntity::Mod(entity)) = self.world.block_entities.get(&pos) else {
                continue;
            };
            let Some(id) = mod_api::__private::block_entity_type_key(entity.id()) else {
                continue;
            };
            let subject = mod_api::__private::game::block_entity_ref(
                level,
                [pos.0, pos.1, pos.2],
                generation,
            );
            if server {
                let mut access = ResidentBlockEntityAccess::on_load(self, subject);
                crate::game::native_hooks::native_block_entity_on_load(
                    mod_api::__private::BlockEntityOnLoadCall {
                        id,
                        subject: BlockEntitySideScope::Server(BlockEntityScope::new(&mut access)),
                    },
                );
            } else {
                let mut access = ClientBlockEntityAccess {
                    game: self,
                    subject,
                };
                crate::game::native_hooks::native_block_entity_on_load(
                    mod_api::__private::BlockEntityOnLoadCall {
                        id,
                        subject: BlockEntitySideScope::Client(BlockEntityClientScope::new(
                            &mut access,
                        )),
                    },
                );
            }
        }
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
