#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, explosion_settings::BlastEffects, lan::LanHost, level_sounds::LevelSound,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_particles::LanExplosion;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_particles::QueuedParticle;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_particles::decode_mod_particle;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_particles::particle_position_source;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_particles::send_snowflake_particle;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_particles::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{SoundSource, client::particles::PositionSource};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::lan::{
    Body, containers::within_range, hub::Hub,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::particles::{
    ExplosionParticle, ParticleOptions, ParticleType,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::{
    entity_data::{Particle, ParticleData, PositionSource as NetworkPositionSource},
    packets::play::{self, Clientbound as C},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::registry::BlockStateId;

impl Game<'_> {
    /// `BucketItem.emptyContents` where water evaporates: `sendParticles(
    /// LARGE_SMOKE, x, y, z, 8, 1, 1, 1, 0, ALTERNATIVE)` from the block's
    /// corner, so each puff lands inside the block. The window draws its own.
    pub(in crate::game) fn evaporation_smoke(&mut self, pos: [i32; 3]) {
        let options = ParticleOptions::simple(ParticleType::LargeSmoke);
        let at = pos.map(f64::from);
        for _ in 0..8 {
            let r = self.particles.0.random();
            let p = [
                at[0] + r.next_double(),
                at[1] + r.next_double(),
                at[2] + r.next_double(),
            ];
            self.particles.0.add(&options, p, [0.0; 3]);
        }
        self.lan_send_particles(QueuedParticle {
            dimension: self.dim.id(),
            options,
            state: None,
            at,
            spread: [1.0; 3],
            speed: [0.0; 3],
            count: 8,
            randomization: 1,
        });
    }
    /// `LivingEntity.checkFallDamage`'s `ServerLevel.sendParticles` of the
    /// landing block's crumbs: drawn in this window, when it has one, and
    /// sent to each player within 32 blocks.
    pub(in crate::game) fn landing_dust(
        &mut self,
        state: BlockStateId,
        at: Vec3,
        count: i32,
        speed: f64,
    ) {
        let at = [at.x, at.y, at.z];
        if self.serving.is_none() && self.options.dedicated.is_none() {
            let block = [
                at[0].floor() as i32,
                (at[1] - 0.2).floor() as i32,
                at[2].floor() as i32,
            ];
            if let Some((sprite, tint)) = self.particle_sprite(state, block) {
                self.particles.0.send_particles(
                    &ParticleOptions::block(ParticleType::Block, sprite, tint)
                        .with_source(&self.res.facts().get(state).name),
                    at,
                    [0.0; 3],
                    speed,
                    count as u32,
                );
            }
        }
        let Some(hub) = self.lan.as_ref().map(|h| &h.hub) else {
            return;
        };
        let Some(particle) =
            hub.particle(&ParticleOptions::simple(ParticleType::Block), Some(state))
        else {
            return;
        };
        let packet = C::LevelParticles(play::LevelParticles {
            particle,
            override_limiter: false,
            always_show: false,
            pos: at,
            dist: [0.0; 3],
            max_speed: [speed as f32; 3],
            count,
            randomization: 0,
        });
        for g in hub.guests.iter().filter(|g| hub.here(g)) {
            // ServerLevel compares the player's block centre, strictly within 32.
            let distance: f64 = (0..3)
                .map(|k| (g.position[k].floor() + 0.5 - at[k]).powi(2))
                .sum();
            if distance < 32.0 * 32.0 {
                g.send(&packet);
            }
        }
    }
    /// `ClientPacketListener.handleExplosion`: the blast's particles (its
    /// block particles as the packet lists them) and its sound, when the
    /// packet carries it, then the player's push.
    pub(in crate::game::client_side) fn remote_explode(&mut self, p: &play::Explode) {
        if p.play_sound {
            let pitch = (1.0
                + (self.particles.0.random().next_float()
                    - self.particles.0.random().next_float())
                    * 0.2)
                * 0.7;
            // The packet's explosionSound: a wind charge's burst, a bed's
            // or anchor's blast, a modded sound.
            let sound = self
                .remote_sound_name(&p.sound)
                .unwrap_or_else(|| "minecraft:entity.generic.explode".to_owned());
            self.audio.0.play(&sound, Some(p.center), 4.0, pitch);
        }
        if let Some(options) = self.received_particle(&p.particle, p.center) {
            self.particles.0.add_with_world(
                Some(&self.world),
                &options,
                p.center,
                [1.0, 0.0, 0.0],
                false,
                false,
            );
        }
        // A wind charge's burst lists none, and one this client can't draw is left out.
        let block_particles: Vec<_> = p
            .block_particles
            .iter()
            .filter_map(|b| {
                Some(ExplosionParticle {
                    options: self.received_particle(&b.particle, p.center)?,
                    scaling: b.scaling,
                    speed: b.speed,
                    weight: u32::try_from(b.weight).ok()?,
                })
            })
            .collect();
        self.particles.0.track_explosion(
            p.center,
            p.radius,
            p.block_count.max(0) as usize,
            &block_particles,
        );
        // `Entity.pushFromExplosion` calls `Entity.push`, which rejects a
        // non-finite impulse before even marking the entity for sync.
        if let (Some([x, y, z]), Some(player)) = (p.knockback, self.player.as_mut()) {
            let impulse = Vec3::new(x, y, z);
            if impulse.is_finite() {
                let velocity = player.velocity.add_vector(impulse);
                // `Entity.setDeltaMovement` can reject the sum, but the push
                // still marks the entity for sync.
                if velocity.is_finite() {
                    player.velocity = velocity;
                }
                player.needs_sync = true;
            }
        }
    }
    /// `ClientPacketListener.handleParticleEvent`: no count is one particle
    /// moving by speed times spread; otherwise each is spread and sped up by
    /// gaussians on each axis.
    pub(in crate::game::client_side) fn remote_level_particles(
        &mut self,
        p: &play::LevelParticles,
    ) {
        let Some(options) = self.received_particle(&p.particle, p.pos) else {
            return;
        };
        // Combat feel Legacy draws a hit as 1.8.9 did: no sweep, no damage hearts.
        if self.pvp_hides_particle(options.ty) {
            return;
        }
        let Some(remote) = &mut self.remote else {
            return;
        };
        self.particles.0.handle_level_particles(
            &mut remote.particle_packet_random,
            &self.world,
            p,
            &options,
        );
    }
    /// Locally draw the level's own particle, then send it to LAN players.
    pub(in crate::game) fn snowflake_particle(&mut self, at: [f64; 3], velocity: [f64; 3]) {
        let options = ParticleOptions::simple(ParticleType::Snowflake);
        if self.options.execution_role.is_client() {
            self.particles.0.add(&options, at, velocity);
        }
        self.lan_send_particles(QueuedParticle {
            dimension: self.dim.id(),
            options,
            state: None,
            at,
            spread: [0.0; 3],
            speed: velocity,
            count: 1,
            randomization: 1,
        });
    }
}
