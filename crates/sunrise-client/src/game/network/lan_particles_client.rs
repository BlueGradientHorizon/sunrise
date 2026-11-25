#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, explosion_settings::BlastEffects, lan::LanHost, level_sounds::LevelSound,
};
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
    /// Client-only landing block crumbs (`particle_sprite` + local particle engine,
    /// e.g. sniffer digging).
    pub(in crate::game) fn client_landing_dust(
        &mut self,
        state: BlockStateId,
        at: Vec3,
        count: i32,
        speed: f64,
    ) {
        assert!(
            !self.is_logical_server(),
            "client_landing_dust called on server role"
        );
        let at = [at.x, at.y, at.z];
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
}
