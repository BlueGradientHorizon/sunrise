#[allow(unused_imports)]
pub(in crate::game::client_side) use super::block_particles::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    blocks::{MAP_COLORS, RenderShape},
    particles::{ParticleOptions, ParticleType},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Direction;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Raw `ClientLevel.addBreakingParticles`: one crumb on the given face, with no sound or hook.
    pub(in crate::game) fn add_breaking_particle(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        face: Direction,
    ) -> bool {
        let Some((sprite, tint)) = self.particle_sprite(state, pos) else {
            return false;
        };
        let boxes = self.particle_shape(pos, state);
        let Some(p) = self.with_lent_level_random(|game| {
            sunrise_client::particles::breaking_position(
                game.side.particles.0.level_random(),
                pos,
                face.normal(),
                &boxes,
            )
        }) else {
            return false;
        };
        let mut particle =
            Particle::terrain(self.side.particles.0.random(), p, [0.0; 3], sprite, tint);
        particle.set_power(0.2);
        particle.scale(0.6);
        self.side.particles.0.push(particle);
        true
    }
    /// `ClientLevel.addDestroyBlockEffect`: decide using the destroyed state, not the block
    /// that now occupies `pos`, then add the default burst per outline box unless handled.
    pub(in crate::game) fn destroy_particles(&mut self, pos: [i32; 3], state: BlockStateId) {
        if self.settings.extras.hide_block_particles || self.res.facts().is_air(state) {
            return;
        }
        let spawn = self.res.facts().get(state).spawn_terrain_particles;
        if self.mod_add_destroy_effects(state, pos).unwrap_or(!spawn) {
            return;
        }
        let Some((sprite, tint)) = self.particle_sprite(state, pos) else {
            return;
        };
        let boxes = self.particle_shape(pos, state);
        sunrise_client::particles::destroy_positions(pos, &boxes, |p, v| {
            let particle =
                Particle::terrain(self.side.particles.0.random(), p, v, sprite.clone(), tint);
            self.side.particles.0.push(particle);
        });
    }
    /// `HoneyBlock.showParticles`: the crumb count of entity event 53 or 54.
    pub(in crate::game::client_side) fn honey_entity_particles(&mut self, at: [f64; 3], event: i8) {
        let Some(count) = sunrise_client::landing::honey_particle_count(event) else {
            return;
        };
        let Some(honey) = self
            .pack
            .assets
            .registry
            .default_state("minecraft:honey_block")
        else {
            return;
        };
        let block = [
            at[0].floor() as i32,
            at[1].floor() as i32,
            at[2].floor() as i32,
        ];
        if let Some((sprite, tint)) = self.particle_sprite(honey, block) {
            for _ in 0..count {
                self.side.particles.0.add(
                    &ParticleOptions::block(ParticleType::Block, sprite.clone(), tint),
                    at,
                    [0.0; 3],
                );
            }
        }
    }
}
