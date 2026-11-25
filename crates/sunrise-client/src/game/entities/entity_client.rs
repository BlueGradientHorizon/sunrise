#[allow(unused_imports)]
pub(in crate::game::client_side) use super::commands::Target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::persist::ChunkKey;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{FallingBlock, Game, general::GeneralRow};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    boats::Boat, fireworks::Rocket, minecarts::Minecart, tnt::PrimedTnt, trident::ThrownTrident,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::Blast;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::DrawExtras;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::DrawFamily;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::DrawRun;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::Entity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::ItemInside;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::JoinOrderError;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::ProjectileInside;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::THROWN_TRIDENT_DIMENSIONS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::ThunderHit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::apply_contact_writes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::armor_stand_size;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::causer_decision;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::damage_level_of;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::define_entity_handles;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::direct_entity_edit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::entity_access;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::entity_lists;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::for_each_entity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::for_each_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::hit_effects_of;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::hit_reads_of;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::list_borrows::entity_lists_of;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::list_borrows::{
    borrow_level_list, borrow_lists,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::load_entity_of_any_kind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::load_lists;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::load_with_transfer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::match_entity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::player_resolver;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::player_victim_causer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::save_entities_of_every_kind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::save_lists;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::stored_ground;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::stored_motion;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::stored_size;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::victim_and_causer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::walk_lists;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::walk_lists_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::with_entity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::with_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::borrow::Cow;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::{HashMap, HashSet};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::collisions::{
    PowderSnowSubject, SceneCollisions,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::entities::{
    ArmorStand, Cushion, ExperienceOrb, ItemEntity, ItemFrame, Painting,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::entity_rules::EntityRules;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::join_seq::JoinSeq;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::join_seq::Joined as _;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::{Mob, Species};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::projectiles::{Projectile, ProjectileKind};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::{Aabb, Direction, Vec3};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::RandomSource as _;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{
    block_collision_scan::CollisionScanError, collision_context::CollisionContext,
    ground_support::GroundState, level::data::PushReaction, move_gate::MoveGate, piston_push,
};

impl Game<'_> {
    /// `Block.fallOn` effects for bodies whose `causeFallDamage` does nothing.
    pub(in crate::game) fn nonliving_fall_on(
        &mut self,
        id: u32,
        feet: Vec3,
        contact: sunrise_client::motion::BodyContact,
    ) {
        // FarmlandBlock draws the level's float before checking LivingEntity.
        match &*self.res.facts().get(contact.state).name {
            "minecraft:farmland" => {
                self.world.random.next_float();
            }
            "minecraft:honey_block" => {
                let at = [feet.x, feet.y, feet.z];
                if !self.entity_silent(id) {
                    self.server_play_sound("minecraft:block.honey_block.slide", at, 1.0, 1.0);
                }
                self.lan_entity_event(id as i32, 54, at);
            }
            _ => {}
        }
    }
}
