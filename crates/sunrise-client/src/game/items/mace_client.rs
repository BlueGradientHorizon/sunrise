#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::FALL_THRESHOLD;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::HEAVY_THRESHOLD;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::KNOCKBACK_POWER;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::KNOCKBACK_RADIUS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::SmashPush;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::Spared;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::ally_of_player;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::breach_modifier;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::can_smash;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::smash_push;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::stack_smash_damage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::sweep_base_damage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mace::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    enchant::{EffectContext, Enchantments},
    health::ArmorWeapon,
    items::{ItemStack, Items},
    mobs::{Damage, DamageKind, Mob, MobEvent, Species},
    particles::{ParticleOptions, ParticleType},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    position::BlockPos,
    random::RandomSource,
};

impl Game<'_> {
    /// The attacker's side of `MaceItem.hurtEnemy` for a smash: the fall
    /// stops, its damage is waived from where the hit landed, and the ground
    /// or air smash sound plays. The fall distance it was, if there is a
    /// player.
    pub(in crate::game) fn mace_smash_self(&mut self, target_on_ground: bool) -> Option<f64> {
        let p = self.player.as_mut()?;
        p.velocity = Vec3::new(p.velocity.x, f64::from(0.01f32), p.velocity.z);
        // A server player's client is told (ClientboundSetEntityMotionPacket): it moves by
        // itself, so without it a Wind Burst's push would add to the speed of the fall.
        p.health.sync_velocity = true;
        // calculateImpactPosition: an earlier impact below the player stays.
        let impact = match p.impulse.impact_position {
            Some(at) if at.y <= p.position.y => at,
            _ => p.position,
        };
        p.impulse.set_ignore_fall_damage(true, Some(impact));
        let fall = p.fall_distance;
        let feet = [p.position.x, p.position.y, p.position.z];
        let sound = if target_on_ground {
            p.spawn_extra_particles_on_fall = true;
            if fall > HEAVY_THRESHOLD {
                "minecraft:item.mace.smash_ground_heavy"
            } else {
                "minecraft:item.mace.smash_ground"
            }
        } else {
            "minecraft:item.mace.smash_air"
        };
        self.audio.0.play(sound, Some(feet), 1.0, 1.0);
        Some(fall)
    }
    /// Level event 2013 (`ParticleUtils.spawnSmashAttackParticles`): dust
    /// pillars of the block at `pos`, a cluster over it and a ring 3.5 out.
    pub(in crate::game) fn smash_dust(&mut self, pos: BlockPos, count: i32) {
        let state = self.world.block(pos.x, pos.y, pos.z);
        let Some((sprite, tint)) = self.particle_sprite(state, [pos.x, pos.y, pos.z]) else {
            return;
        };
        let options = ParticleOptions::block(ParticleType::DustPillar, sprite, tint);
        let center = [
            f64::from(pos.x) + 0.5,
            f64::from(pos.y) + 1.0,
            f64::from(pos.z) + 0.5,
        ];
        let near = f64::from(0.2f32);
        let mut i = 0;
        while (i as f32) < count as f32 / 3.0 {
            let r = self.particles.0.random();
            let x = center[0] + r.next_gaussian() / 2.0;
            let z = center[2] + r.next_gaussian() / 2.0;
            let v = [
                r.next_gaussian() * near,
                r.next_gaussian() * near,
                r.next_gaussian() * near,
            ];
            self.particles.0.add(&options, [x, center[1], z], v);
            i += 1;
        }
        let far = f64::from(0.05f32);
        let mut i = 0;
        while (i as f32) < count as f32 / 1.5 {
            let r = self.particles.0.random();
            let angle = f64::from(i);
            let x = center[0] + KNOCKBACK_RADIUS * angle.cos() + r.next_gaussian() / 2.0;
            let z = center[2] + KNOCKBACK_RADIUS * angle.sin() + r.next_gaussian() / 2.0;
            let v = [
                r.next_gaussian() * far,
                r.next_gaussian() * far,
                r.next_gaussian() * far,
            ];
            self.particles.0.add(&options, [x, center[1], z], v);
            i += 1;
        }
    }
}
