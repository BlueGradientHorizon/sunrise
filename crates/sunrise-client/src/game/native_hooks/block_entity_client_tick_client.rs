#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::api_values::TryIntoApi;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::block_entity_client_tick::ClientBlockEntityAccess;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::block_entity_client_tick::ResidentReadBlockEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::block_entity_client_tick::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    BlockEntityClientAccess, BlockEntityClientScope, BlockEntityReadView, BlockEntityRef,
    ClientHost, ResidentBlockEntityReadOnly,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::animate_tick::campfire_particle_tick;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    block_entities::Ticking, containers::BlockEntity,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Compound;

impl Game<'_> {
    /// `CampfireBlockEntity.particleTick` of the campfire at `key`.
    pub(in crate::game) fn client_tick_campfire(&mut self, (x, y, z): (i32, i32, i32)) {
        let cooking = match self.world.block_entities.get(&(x, y, z)) {
            Some(BlockEntity::Campfire(c)) => c.items.each_ref().map(Option::is_some),
            _ => return,
        };
        let state = self.world.block(x, y, z).0;
        campfire_particle_tick(
            self.res.animate(),
            &mut self.particles.0,
            self.pack.assets.registry,
            [x, y, z],
            state,
            cooking,
        );
    }
}
