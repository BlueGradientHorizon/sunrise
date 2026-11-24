#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Dim, Game};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::respawn_anchor::AnchorUse;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::respawn_anchor::HORIZONTAL;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::respawn_anchor::RespawnConfig;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::respawn_anchor::SavedRespawn;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::respawn_anchor::look_at_yaw;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::respawn_anchor::respawn_offsets;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::respawn_anchor::wrap_degrees;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::respawn_anchor::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::client::first_person::Hand;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    blocks::Fluid,
    collisions::{SceneCollisions, no_collision_empty},
    items::ItemStack,
    mobs::Who,
    stand_up::{StandUpLevel, StandUpType, bed_forward, stand_up_fallback},
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Axis, Vec3, VoxelShape},
    position::BlockPos,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{
    collision_context::CollisionContext, level::data::state_flags, registry::BlockStateId,
};

impl Game<'_> {
    /// `RespawnAnchorBlock.charge`: one more charge, the block change and the sound.
    pub(in crate::game) fn charge_anchor(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        who: Option<Who>,
    ) -> Result<()> {
        let Some(next) = self.anchor_with_charges(state, self.anchor_charges(state) + 1) else {
            return Ok(());
        };
        self.level_set(pos, next)?;
        self.post_block_game_event("minecraft:block_change", pos, who, Some(next));
        let at = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + 0.5,
            f64::from(pos[2]) + 0.5,
        ];
        self.audio
            .0
            .play("minecraft:block.respawn_anchor.charge", Some(at), 1.0, 1.0);
        Ok(())
    }
    /// `StrawBedBlock.destroyBed`: the break sound, then the bed is air.
    pub(in crate::game) fn destroy_straw_bed(&mut self, head: [i32; 3]) -> Result<()> {
        let center = [
            f64::from(head[0]) + 0.5,
            f64::from(head[1]) + 0.5,
            f64::from(head[2]) + 0.5,
        ];
        self.audio.0.play(
            "minecraft:block.straw_bed.break_leave",
            Some(center),
            1.0,
            1.0,
        );
        self.level_set(head, self.world.air)
    }
}
