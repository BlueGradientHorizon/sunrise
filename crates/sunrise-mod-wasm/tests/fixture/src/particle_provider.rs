//! A particle provider whose batch tick crosses the wasm boundary: the batch goes in, every
//! particle's writes come back, and the level view is called from the guest.
use mod_api::{
    LightLayer, ParticleBatch, ParticleFacing, ParticleLevelView, ParticleProvider,
    ParticleProviders,
};
use std::sync::Arc;

pub const KIND: &str = "wasm_fixture:spark";

pub fn register(providers: &mut dyn ParticleProviders) {
    providers.register(KIND.into(), Arc::new(Spark));
}

/// Moves each particle by its place in the batch, keeps a count in its state, lays it flat with a
/// roll, and colours it by the level's light at its own block.
struct Spark;

impl ParticleProvider for Spark {
    fn tick(&self, particles: &mut ParticleBatch, level: &dyn ParticleLevelView) {
        for (index, particle) in particles.iter_mut().enumerate() {
            particle.position[0] += 1.0 + index as f64;
            particle.state[0] += 1.0;
            particle.facing = ParticleFacing::Fixed([-0.5, -0.5, -0.5, 0.5]);
            particle.roll = 0.25 * (index as f32 + 1.0);
            let at = particle.position.map(|c| c.floor() as i32);
            particle.rgba[0] = f32::from(level.brightness(LightLayer::Sky, at)) / 15.0;
            particle.rgba[1] = f32::from(level.brightness(LightLayer::Block, at)) / 15.0;
            particle.removed = particle.age > 2;
        }
    }
}
