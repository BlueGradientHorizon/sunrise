#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game, Screen, native_mob_draw::PlayerRender, shoulders::shoulder_variants,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::PLAYER_TYPES;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::PreviewGlint;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::RiddenBody;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::VisualRays;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::body_visible;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::emit_preview;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::entity_in_ui_shade;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::entity_light_block;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::load_player_type;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::outline_offset;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::own_invisible;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::player_model_positions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::player_preview_quads;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::player_preview_quads_no_cull;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::player_type;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::player_type_with_document;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::player_walk_distance;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::preview_glint_quads;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::ray_interaction_shape;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::ray_shape_from_boxes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::rope_old_after_turn;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::shadow_pass_takes_own_model;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::turn_rotation;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::wrap;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::perspective::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::Hand;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    blocks::BlockFacts,
    camera::{CameraEntity, CameraType, LivingCamera},
    collisions::SceneCollisions,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    entities::nameplate::NameplateDistances,
    hud::HudBatch,
    mobs::{Anim, Brain, Mob, MobType, render::FrameDraw},
    mouse::Rotation,
    pose::Pose,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::{Aabb, ShapeOp, Vec3, VoxelShape},
    position::BlockPos,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::{
    collision_context::{CollisionContext, CollisionEntity, CollisionItem},
    entity_obstruction::EntityIdentity,
    registry::BlockStateId,
    world_ray::{RayBlockMode, RayFluidMode, RayWorld, clip_world},
};

impl Game<'_> {
    /// Arrows and bee stingers stuck in the local player's body.
    #[inline(always)]
    pub(in crate::game) fn local_stuck_counts(&self) -> (i32, i32) {
        (self.combat.0.stuck.arrows, self.combat.0.stuck.stingers)
    }
}
