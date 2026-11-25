#[allow(unused_imports)]
pub(in crate::game::server_side) use super::block_entity_client_tick::ClientBlockEntityAccess;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::block_entity_tick::ResidentBlockEntityAccess;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_on_load::authoritative_world_for_mods;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_on_load::native_block_entity_on_load;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_on_load::world_for_mods;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::block_entity_on_load::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    BlockEntityClientScope, BlockEntityScope, BlockEntitySideScope,
};

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn server_fire_pending_block_entity_loads(&mut self) {
        self.fire_pending_block_entity_data_loads();
        if !self.world.block_entities.has_pending_on_load() {
            return;
        }
        let Some(level) = self.level(self.dim.id()) else {
            return;
        };
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
            let mut access = ResidentBlockEntityAccess::on_load(self, subject);
            crate::game::native_hooks::native_block_entity_on_load(
                mod_api::__private::BlockEntityOnLoadCall {
                    id,
                    subject: BlockEntitySideScope::Server(BlockEntityScope::new(&mut access)),
                },
            );
        }
    }

    /// A replacement can inherit a ticker slot later in the current pass.
    /// Deliver its attachment notice before lending its resident row to that tick.
    /// Returns whether the resident ticker was dispatched.
    pub(in crate::game) fn tick_mod_block_entity(&mut self, pos: Pos) -> bool {
        if mod_api::__private::any_mod_disabled()
            && let Some(BlockEntity::Mod(entity)) = self.world.block_entities.get(&pos)
            && mod_api::__private::content_owner_disabled(entity.id())
        {
            return false;
        }
        if self.world.block_entities.has_pending_on_load()
            || self.world.block_entities.has_pending_data_load()
        {
            let generation = self.world.block_entities.generation(&pos);
            self.fire_pending_block_entity_loads();
            if self.world.block_entities.generation(&pos) != generation {
                return false;
            }
        }
        self.tick_mod_block_entity_loaded(pos)
    }
}
