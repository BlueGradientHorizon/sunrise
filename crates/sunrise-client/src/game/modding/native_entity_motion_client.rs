#[allow(unused_imports)]
pub(in crate::game::client_side) use super::entity::{
    Entity as _, EntityHandle, with_entity, with_entity_mut,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, commands::Target};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_entity_motion::corners;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_entity_motion::on_block_position;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_entity_motion::ray_end;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_entity_motion::turned_view_vector;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_entity_motion::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    ClipBlock, ClipFluid, ClipHit, EntityRef, LevelWriteError,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    collisions::SceneCollisions, motion::player_context,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::{Aabb, Axis, Vec3};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::BlockPos;

impl Game<'_> {
    /// The height of the entity's eyes above its feet (`Entity.getEyeHeight`).
    /// API 379.
    pub(in crate::game) fn entity_eye_height(&self, entity: EntityRef) -> Option<f64> {
        if let Some(target) = self.native_entity_read_target(entity) {
            return Some(self.target_eye_height(target));
        }
        let (remote, id) = self.native_client_entity_context(entity)?;
        remote.tracked(id)?;
        self.client
            .interactions
            .get(&id)
            .map(|i| f64::from(i.eye_height()))
    }
}
