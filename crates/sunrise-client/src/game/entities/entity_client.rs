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

/// Bounds and identity needed by the render occlusion culler. Moving renderers
/// use interpolated entity bounds; attached entities use their own world bounds.
pub(in crate::game) trait RenderEntityBounds {
    fn render_id(&self) -> u32;
    fn render_bounds(&self, alpha: f64) -> Aabb;
    fn render_glowing(&self, game: &Game<'_>) -> bool {
        game.tracked_entity_glowing(self.render_id())
    }
}

impl RenderEntityBounds for Mob {
    fn render_id(&self) -> u32 {
        self.id
    }

    fn render_bounds(&self, alpha: f64) -> Aabb {
        let (width, height, _) = self.awake_dims();
        let at = self.previous.lerp(self.position, f64::from(alpha as f32));
        if matches!(
            self.species(),
            Species::Guardian | Species::ElderGuardian | Species::Phantom
        ) {
            let root_scale = self.kind.model.pose[6..]
                .iter()
                .copied()
                .map(f32::abs)
                .fold(1.0, f32::max);
            let radius = width.max(height) * 2.5 + 3.0 * f64::from(root_scale * self.model_scale());
            return Aabb::of_size(at, Vec3::new(radius * 2.0, radius * 2.0, radius * 2.0));
        }

        let horizontal = (width * 2.0).max(height * 1.5).max(3.5);
        let vertical = height * 3.0 + 0.5;
        Aabb::new(
            at.subtract(Vec3::new(horizontal, vertical, horizontal)),
            at.add_vector(Vec3::new(horizontal, vertical, horizontal)),
        )
    }

    fn render_glowing(&self, _game: &Game<'_>) -> bool {
        self.is_currently_glowing()
    }
}

impl RenderEntityBounds for ArmorStand {
    fn render_id(&self) -> u32 {
        self.id
    }

    fn render_bounds(&self, alpha: f64) -> Aabb {
        let at = self.previous.lerp(self.position, alpha);
        let scale = if self.small { 0.5 } else { 1.0 };
        let equipped = self.equipment.iter().any(Option::is_some);
        let horizontal = if equipped { 2.0 } else { 1.5 * scale };
        let lower = if equipped { -1.0 } else { -0.5 * scale };
        let upper = 3.5 * scale
            + if self.equipment[5].is_some() {
                1.5
            } else {
                0.0
            };
        Aabb::new(
            at.subtract(Vec3::new(horizontal, -lower, horizontal)),
            at.add_vector(Vec3::new(horizontal, upper, horizontal)),
        )
    }
}

impl RenderEntityBounds for ItemEntity {
    fn render_id(&self) -> u32 {
        self.id
    }

    fn render_bounds(&self, alpha: f64) -> Aabb {
        let at = self.previous.lerp(self.position, alpha);
        Aabb::new(
            at.subtract(Vec3::new(0.75, 0.1, 0.75)),
            at.add_vector(Vec3::new(0.75, 1.0, 0.75)),
        )
    }
}

impl RenderEntityBounds for Boat {
    fn render_id(&self) -> u32 {
        self.id
    }

    fn render_bounds(&self, alpha: f64) -> Aabb {
        let at = self.previous.lerp(self.position, alpha);
        Aabb::new(
            at.subtract(Vec3::new(1.75, 0.75, 1.75)),
            at.add_vector(Vec3::new(1.75, 1.5, 1.75)),
        )
    }
}

impl RenderEntityBounds for Minecart {
    fn render_id(&self) -> u32 {
        self.id
    }

    fn render_bounds(&self, alpha: f64) -> Aabb {
        let at = self.previous.lerp(self.position, alpha);
        let offset = self.custom_offset.map_or_else(
            || {
                if self.kind == super::minecarts::CartKind::Hopper {
                    1.0
                } else {
                    6.0
                }
            },
            f64::from,
        );
        let display_shift = 0.75 * (offset - 8.0) / 16.0;
        let display_low = at.y + 0.375 - 0.4 + display_shift;
        let display_high = at.y + 0.375 + 0.4 + display_shift + 0.9;
        Aabb::new(
            Vec3::new(at.x - 1.25, (at.y - 0.5).min(display_low), at.z - 1.25),
            Vec3::new(at.x + 1.25, (at.y + 1.0).max(display_high), at.z + 1.25),
        )
    }
}

fn aabb_from_extents(bounds: ([f64; 3], [f64; 3])) -> Aabb {
    Aabb::new(
        Vec3::new(bounds.0[0], bounds.0[1], bounds.0[2]),
        Vec3::new(bounds.1[0], bounds.1[1], bounds.1[2]),
    )
}

impl RenderEntityBounds for Painting {
    fn render_id(&self) -> u32 {
        self.id
    }

    fn render_bounds(&self, _alpha: f64) -> Aabb {
        aabb_from_extents(self.bounds())
    }
}

impl RenderEntityBounds for ItemFrame {
    fn render_id(&self) -> u32 {
        self.id
    }

    fn render_bounds(&self, _alpha: f64) -> Aabb {
        let (lo, hi) = self.bounds();
        let step = self.step().map(f64::from);
        let center: [f64; 3] = std::array::from_fn(|axis| (lo[axis] + hi[axis]) * 0.5);
        // Include both maps and protruding held items in the occlusion envelope.
        let half: [f64; 3] = std::array::from_fn(|axis| if step[axis] != 0.0 { 0.75 } else { 0.5 });
        aabb_from_extents((
            std::array::from_fn(|axis| center[axis] - half[axis]),
            std::array::from_fn(|axis| center[axis] + half[axis]),
        ))
    }
}

impl RenderEntityBounds for Cushion {
    fn render_id(&self) -> u32 {
        self.id
    }

    fn render_bounds(&self, _alpha: f64) -> Aabb {
        aabb_from_extents(self.bounds())
    }
}

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
