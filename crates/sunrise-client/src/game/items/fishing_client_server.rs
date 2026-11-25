#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game, billboard_axes, creatures::ProjectileHitter, lan_mobs::guest_bounds,
};
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::fishing_client::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::EntityCollision;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::FISHING_TABLE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::Hook;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::HookLerp;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::HookMedium;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::HookTarget;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::RemoteHook;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::SIZE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::State;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::WARNED_TABLES;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::bite_impulse;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::near_plane_hand;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::push_remote_hook;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::remove_remote_hook_from;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::synchronized_random;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_side::fishing::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::advancements::EntityView;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::lan::entity::States;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    blocks::Fluid,
    collisions::SceneCollisions,
    gizmos::Line,
    items::ItemStack,
    mesher::Vertex,
    particles::{ParticleOptions, ParticleType},
    pose::Pose,
    random_source,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::{Aabb, BlockHit, Vec3, traverse_blocks, trig},
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::entity_data::{Value, serializer};

impl Game<'_, crate::game::server_side::ServerState> {
    /// `ServerLevel.sendParticles` of a simple particle.
    pub(in crate::game) fn send_particles(
        &mut self,
        kind: ParticleType,
        at: [f64; 3],
        spread: [f64; 3],
        speed: f64,
        count: u32,
    ) {
        self.server_send_particles(crate::lan::server_effects::ServerQueuedParticle {
            dimension: self.dim.id(),
            particle: crate::lan::server_effects::ServerParticle::simple(kind),
            at,
            spread,
            speed: [speed; 3],
            count: count as i32,
            randomization: 0,
            except: None,
        });
    }
}
