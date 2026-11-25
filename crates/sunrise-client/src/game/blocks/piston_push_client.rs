#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game,
    entity::{Entity, EntityHandle, with_entity, with_entity_mut},
    minecarts::CartHit,
    move_gate::MoveWho,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::collision_source::Mover;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_push::Body;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_push::Lid;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_push::PistonIndex;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_push::Push;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_push::PushTarget;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_push::ShulkerPush;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_push::StepHeights;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_push::first_axis_direction;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_push::pushed_movement;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_push::shulker_lid;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::piston_push::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::collisions::{
    EntityCollisions, SceneCollisions, no_collision_empty, with_piston_noclip,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::entity_rules::EntityRules;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::join_seq::{JoinSeq, Joined};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::Species;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::motion::MotionWorld;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::{
    Aabb, Axis, COLLISION_EPSILON, Direction, ShapeOp, Vec3, border::BorderGeometry,
    entity_collision_bounds,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{
    block_collision_scan::BlockCollisionWorld, collision_context::CollisionContext,
    entity_move::MoverType, level::data::PushReaction, level::pistons::MovingPiston,
    move_gate::MoveGate, piston_movement::PistonMovement, piston_push,
    world_movement::EntityMovementWorld,
};

impl Game<'_> {
    /// A lid's mover is simulated on both sides: the client displaces its
    /// player and all boats the lid meets, not only the boat it drives.
    /// Restitution still belongs to the side simulating each boat.
    pub(in crate::game) fn lid_push_local(&mut self, lid: Lid, area: Aabb, movement: Vec3) {
        let root = self.lid_root(lid);
        let boats: Vec<_> = self
            .boats
            .iter()
            .enumerate()
            .filter(|(_, boat)| boat.bounds().intersects(area))
            .filter_map(|(i, _)| self.stable_push_target(MoveWho::Entity(EntityHandle::Boat(i))))
            .collect();
        if self
            .player
            .as_ref()
            .is_some_and(|p| !p.spectator && p.bounds().intersects(area))
            && !self.lid_spares(lid, root, MoveWho::Player, false)
        {
            let _ = self.move_local_player(lid.mover(), movement, false);
        }
        for boat in boats {
            let Some(target @ MoveWho::Entity(handle)) = self.resolve_push_target(boat) else {
                continue;
            };
            let body = self.listed_body(handle, None);
            if lid == Lid::Box && body.piston_reaction == PushReaction::IgnoreEntity {
                continue;
            }
            let no_physics = self.pushed_no_physics(target);
            if !self.lid_spares(lid, root, target, no_physics) {
                self.move_pushed_body(lid.mover(), target, body, movement, no_physics, &mut None);
            }
        }
    }
    /// [`Self::lid_push_local`] for this tick's shulker box lids.
    pub(in crate::game) fn push_shulker_local(&mut self, pushes: Vec<ShulkerPush>) {
        for (_, facing, lid) in pushes {
            let (movement, area) = shulker_lid(facing, lid);
            self.lid_push_local(Lid::Box, area, movement);
        }
    }
}
