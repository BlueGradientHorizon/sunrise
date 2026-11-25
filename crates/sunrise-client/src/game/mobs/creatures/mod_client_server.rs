#[allow(unused_imports)]
pub(in crate::game::server_side) use super::api_values::{TryIntoApi, TryIntoEngine};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::level_sounds::LevelSound;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::piston_push::Lid;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Dim, Game, InteractTarget,
    entity_culling::EntityCullDecision,
    native_mob_draw::PlayerRender,
    native_projectiles::{Impact, PendingImpact, ProjectileHit, projectile_impact_event},
    view_vector,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::creatures_client::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::Creatures;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::DyingDrops;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::Finalized;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::MobDrawFrame;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::MobOutputBatch;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::NativeSound;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::NormalActionFrame;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::NormalMobActions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::OrbOrder;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::creatures::PICKABLE_PROJECTILE_IDS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::PendingMobBatch;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::ProjectileHandBack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::ProjectileHitter;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::RemainingActions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::Skinned;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::SpawnReason;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::SpawnSite;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::arrow_miss;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::bounced_place;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::breeding_credit;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::bucketable;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::death_source::death_hurt_source;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::dye_color_mix_in_recipes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::dye_mix;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::flame_geometry;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::from_loot;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::is_item_billboard;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::killed_by_mob;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::living_draw::LivingFrame;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::local_difficulty_in;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::mob_effect_instance;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::named_mob_pickable;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::creatures::pickable_projectile_hit;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::creatures::pickable_projectile_hitboxes;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::creatures::pickable_projectile_hitboxes_into;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::projectile_targets::{
    NonMobHit, ProjectileHitInfo, TargetQuery, is_pickable_projectile,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::seen;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::thrown_item_axes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::thrown_item_quad;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::to_loot;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{EquipmentSlot, LootDrops, SoundSource};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::health::{
    damage_type, melee, source_damage_type,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    Result,
    arrows::EntityHit,
    blocks::Fluid,
    crafting::Recipes,
    fast_hash::FxHashMap,
    graphics::EntityCullingMode,
    health::{Attack, HealthEvent, HurtSource, Hurting, Knock},
    items::{ItemStack, Items},
    join_seq::JoinSeq,
    mesher::Vertex,
    mobs::{
        Damage, DamageKind, Mob, MobEvent, MobWorld, PlayerView, Seen, SpawnSetup, Species, Who,
        ai::{eating_sound, eating_sound_is_bound, feed_animal, takes_food},
        entity_sounds::full_id,
        finalize::Group,
        spawning::{self, BiomeFacts},
    },
    particles::{ParticleOptions, ParticleType},
    projectiles::{Hit, Projectile, ProjectileKind},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_render::render_geometry::EntitySpan;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::{Aabb, Direction, Vec3, trig::Mth},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::projectile_world::ProjWorld;

impl Game<'_> {
    /// `Armadillo.mobInteract`: a brush brushes a scute off an adult
    /// (`brushOffScute`) and loses 16 durability.
    pub(in crate::game) fn interact_armadillo(&mut self, index: usize, item: &str) -> bool {
        let m = &self.mobs[index];
        if item != "minecraft:brush"
            || m.species() != Species::Armadillo
            || !m.alive()
            || m.is_baby()
        {
            return false;
        }
        let (id, at) = (m.id, m.position);
        let feet = [at.x, at.y, at.z];
        let tool = self.held().cloned();
        let table = "minecraft:brush/armadillo";
        let mut stacks = self.sequence_loot(table, table, tool.as_ref());
        let kind = super::native_hooks::mob_kind(&self.mobs[index]);
        let player = Some(self.native_acting_id());
        self.native_use_drops(
            table,
            feet,
            None,
            Some((id, &kind)),
            player,
            tool.as_ref(),
            &mut stacks,
        );
        for d in stacks {
            self.spawn_at_location(at, 0.0, d);
        }
        self.server_play_sound("minecraft:entity.armadillo.brush", feet, 1.0, 1.0);
        let ctx = self.who_context(Some(Who::Mob(id)), None);
        self.post_game_event("minecraft:entity_interact", feet, ctx);
        // ItemStack.hurtAndBreak(16).
        if !self.creative
            && let Some(stack) = tool
        {
            let max = self
                .res
                .items()
                .get(&stack.item)
                .map_or(0, |i| i.max_damage);
            if max > 0 {
                self.damage_held(16);
            }
        }
        true
    }
}
