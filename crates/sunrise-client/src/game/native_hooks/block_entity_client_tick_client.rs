use super::*;
use sunrise_client::animate_tick::campfire_particle_tick;
use sunrise_client::containers::BlockEntity;

impl Game<'_, crate::game::client_side::ClientState> {
    /// `CampfireBlockEntity.particleTick` of the campfire at `key`.
    pub(in crate::game) fn client_tick_campfire(&mut self, (x, y, z): (i32, i32, i32)) {
        let cooking = match self.world.block_entities.get(&(x, y, z)) {
            Some(BlockEntity::Campfire(c)) => c.items.each_ref().map(Option::is_some),
            _ => return,
        };
        let state = self.world.block(x, y, z).0;
        campfire_particle_tick(
            crate::game::client_side::ClientState::expect_client_res(&self.side.client_res)
                .animate(),
            &mut self.side.particles.0,
            self.pack.assets.registry,
            [x, y, z],
            state,
            cooking,
        );
    }
}
