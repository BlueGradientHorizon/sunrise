#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game,
    item_draw::{BlockItemDraw, block_entity_pose},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vault::Players;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vault::Pos;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vault::TakenVault;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vault::VaultEject;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::vault::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    block_entities::BlockEntityLease,
    block_entity_render::vertex_light,
    camera::rotate_degrees,
    containers::BlockEntity,
    item_render,
    items::ItemStack,
    mesher::Vertex,
    vault::{Vault, VaultEvent, VaultLevel, VaultPlayer, VaultState},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client_render::block_entity_reach::Reach;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::{LegacyRandom, LevelRandom};

impl Game<'_> {
    pub(in crate::game) fn vault_events(
        &mut self,
        key: Pos,
        events: Vec<VaultEvent>,
    ) -> Result<()> {
        let pos = [key.0, key.1, key.2];
        let center = pos.map(|v| f64::from(v) + 0.5);
        for event in events {
            match event {
                VaultEvent::State(state) => self.set_vault_state(key, state)?,
                VaultEvent::BlockUpdated => self.mark_block_entity_updated(key),
                VaultEvent::LevelEvent(id, data) => self.server_level_event_at(id, pos, data),
                VaultEvent::Sound(name, volume, pitch) => {
                    self.server_play_sound(&format!("minecraft:{name}"), center, volume, pitch)
                }
                // Vec3.atBottomCenterOf(pos).relative(UP, 1.2), lowered by spawnItem.
                VaultEvent::Eject(stack, v) => {
                    self.native_vault_loot(pos, &stack.item);
                    self.spawn_item_thrown(
                        stack,
                        [center[0], center[1] + 0.7 - 0.125, center[2]],
                        v,
                    )
                }
            }
        }
        Ok(())
    }
}
