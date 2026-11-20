//! Code-defined particle providers, as a client mod registers them in `register_particle_providers`
//! (NeoForge's `RegisterParticleProvidersEvent` and `ParticleProvider`).
//!
//! A provider owns one of the mod's own particle types, named as `ParticleOptions::named` names it
//! (`mymod:spark`). The particle is built as a declarative one is (the sprite set and the
//! `sunrise:behaviour` of `assets/<namespace>/particles/<path>.json` give its spawn values) and the
//! provider then moves it every tick. The game resolves a type's provider once, when the particle
//! sprite sets load, so a tick goes to the provider that owns the particle and to no other.
use crate::{BlockState, LightLayer};
use std::{collections::BTreeMap, sync::Arc};

/// One particle of a provider's type, as its tick reads and writes it: the fields of vanilla's
/// `Particle` a `tick` moves. Plain numbers, so a tick allocates nothing, and one value for
/// native and wasm mods alike.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the game builds it for each tick and a mod only edits its fields
#[derive(sunrise_mod_api_macros::Wire)]
pub struct ModParticle {
    /// `Particle.x/y/z`: the position now. A tick moves the particle by writing it.
    pub position: [f64; 3],
    /// `Particle.xo/yo/zo`: where it was a tick ago, which a frame interpolates from. The game
    /// sets it to `position` before each tick; a tick that teleports the particle sets it too.
    pub previous: [f64; 3],
    /// `Particle.xd/yd/zd`: the velocity the spawn gave it (the declared one, or the spawn
    /// call's). The game applies no vanilla motion to a provider's particle, so a tick moves
    /// `position` by it, or not.
    pub velocity: [f64; 3],
    /// `Particle.age`, the ticks this particle has run: 1 on its first tick. The game advances it
    /// before each tick.
    pub age: i32,
    /// `Particle.lifetime`: the particle expires on the tick after `age` reaches it. A tick may
    /// change it.
    pub lifetime: i32,
    /// `SingleQuadParticle.quadSize`: the billboard's half-width in blocks.
    pub size: f32,
    /// `Particle.rCol/gCol/bCol/alpha`, each 0 to 1.
    pub rgba: [f32; 4],
    /// The sprite drawn, an index in the sprite set `particles/<path>.json` lists (the last one
    /// when it is past the list).
    pub frame: u32,
    /// `Particle.remove()`: set it to expire the particle before its lifetime is up.
    pub removed: bool,
    /// Four numbers that stay with this particle from one tick to the next, zero when it
    /// spawns.
    pub state: [f64; 4],
    /// How the quad turns toward the viewer (`SingleQuadParticle.getFacingCameraMode`).
    pub facing: ParticleFacing,
    /// `SingleQuadParticle.roll`: the quad's turn about its own normal, in radians. The game
    /// interpolates it from the value the previous tick left, as vanilla's `oRoll` does.
    pub roll: f32,
}

/// How a provider's particle quad is oriented (`SingleQuadParticle.FacingCameraMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
// exhaustive: a closed set the game matches without a wildcard
#[derive(sunrise_mod_api_macros::Wire)]
pub enum ParticleFacing {
    /// `LOOKAT_XYZ`: the quad faces the camera. The default.
    #[default]
    Camera,
    /// `LOOKAT_Y`: the quad turns about the Y axis only, to face the camera's side.
    AroundY,
    /// A world-fixed orientation, as a unit quaternion `[x, y, z, w]` turning the quad's
    /// default plane (corners in XY, facing +Z). A quad lying flat facing up is
    /// `[-sin(pi/4), 0, 0, cos(pi/4)]`. `roll` turns the quad about its own normal after it.
    Fixed([f32; 4]),
}

/// The particles of one provider's batch, in a fixed number and order: a tick edits them and
/// cannot add or remove one (`ModParticle::removed` expires a particle).
#[derive(Clone, Debug, PartialEq)]
// exhaustive: its field is private; the game builds it
#[derive(sunrise_mod_api_macros::Wire)]
pub struct ParticleBatch(Vec<ModParticle>);

impl ParticleBatch {
    /// How many particles the batch holds.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Whether the batch holds none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// The particle at `index`.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut ModParticle> {
        self.0.get_mut(index)
    }
    /// Every particle, in order.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, ModParticle> {
        self.0.iter_mut()
    }
}
/// A batch of `particles`, built by the game; a mod cannot make or replace one.
pub fn particle_batch_from_vec(particles: Vec<ModParticle>) -> ParticleBatch {
    ParticleBatch(particles)
}
/// The particles back, for the game to reuse their storage.
pub fn particle_batch_into_vec(batch: ParticleBatch) -> Vec<ModParticle> {
    batch.0
}
impl<'a> IntoIterator for &'a mut ParticleBatch {
    type Item = &'a mut ModParticle;
    type IntoIter = std::slice::IterMut<'a, ModParticle>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

/// What a provider's tick reads of the level around its particles: the loaded world as
/// `Particle` code sees it (`level.getBlockState`, `level.getBrightness`). Read-only. In a chunk
/// that is not loaded `block_state` answers the game's air and `brightness` answers 15 for the sky
/// layer and 0 for the block layer.
pub trait ParticleLevelView {
    /// `BlockGetter.getBlockState`: the state at `pos`.
    fn block_state(&self, pos: [i32; 3]) -> BlockState;
    /// `BlockAndLightGetter.getBrightness`: the stored light of one layer at `pos`, 0 to 15.
    fn brightness(&self, layer: LightLayer, pos: [i32; 3]) -> u8;
}

/// A mod's particle provider: NeoForge's `ParticleProvider`, held by the game for every particle
/// of its type. Particles of a type share one provider, so per-particle data lives in
/// [`ModParticle::state`], not in the provider.
///
/// A tick has no `Host`: the engine ticks particles inside the render crate, which reaches no
/// gameplay. It does read the level through [`ParticleLevelView`].
pub trait ParticleProvider: Send + Sync + 'static {
    // hidden: it marks the trait's methods as imports of a mod that implements it; nothing calls it.
    #[doc(hidden)]
    fn __sunrise_marks(&self, _: crate::__private::Sealed) {
        sunrise_mod_marks::particle_provider::<Self>();
    }
    /// `Particle.tick` for every live particle of this provider's type: one call per type per
    /// tick, native and wasm alike, with the whole batch. The game has set `previous` and `age`
    /// on each already and expires a particle once `age` passes `lifetime`; the tick sets
    /// `position`, `frame`, `size`, `rgba`, `lifetime`, `removed`, `facing` and `roll`. A number
    /// that is not finite expires the particle. A tick edits the batch's particles and cannot
    /// add or remove one. Not called when no particle of the type is alive.
    fn tick(&self, particles: &mut ParticleBatch, level: &dyn ParticleLevelView);
}

/// The particle provider registry `register_particle_providers` hands out: NeoForge's
/// `RegisterParticleProvidersEvent`.
pub trait ParticleProviders {
    /// `registerSpriteSet(type, provider)`: `provider` moves every particle of the type `name`
    /// (`mymod:spark`), whose sprite set is `assets/mymod/particles/spark.json`. A later
    /// registration for a name replaces the earlier one. A name no sprite set matches is logged
    /// and its particles draw nothing, as any `named` type without its sprite set does.
    fn register(&mut self, name: String, provider: Arc<dyn ParticleProvider>);
}

// no wire: process-local listener or callback storage; trait objects are invoked through host dispatch, not serialized.
/// The game's [`ParticleProviders`]: the registrations by name.
#[derive(Default)]
// exhaustive: its field is private; the game builds it
pub struct ParticleProviderRegistry {
    providers: BTreeMap<String, Arc<dyn ParticleProvider>>,
}

impl ParticleProviders for ParticleProviderRegistry {
    fn register(&mut self, name: String, provider: Arc<dyn ParticleProvider>) {
        self.providers.insert(name, provider);
    }
}

impl ParticleProviderRegistry {
    /// Each type's provider, the last registration for a name winning.
    pub fn into_providers(self) -> BTreeMap<String, Arc<dyn ParticleProvider>> {
        self.providers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Drift(f64);
    impl ParticleProvider for Drift {
        fn tick(&self, particles: &mut ParticleBatch, _: &dyn ParticleLevelView) {
            for particle in particles {
                particle.position[0] += self.0;
            }
        }
    }

    fn particle() -> ModParticle {
        ModParticle {
            position: [0.0; 3],
            previous: [0.0; 3],
            velocity: [0.0; 3],
            age: 1,
            lifetime: 20,
            size: 0.1,
            rgba: [1.0; 4],
            frame: 0,
            removed: false,
            state: [0.0; 4],
            facing: ParticleFacing::default(),
            roll: 0.0,
        }
    }

    struct Air;
    impl ParticleLevelView for Air {
        fn block_state(&self, _: [i32; 3]) -> BlockState {
            unreachable!("the test providers read no blocks")
        }
        fn brightness(&self, _: LightLayer, _: [i32; 3]) -> u8 {
            0
        }
    }

    #[test]
    fn a_later_registration_for_a_name_replaces_the_earlier_one() {
        let mut registry = ParticleProviderRegistry::default();
        registry.register("mymod:a".into(), Arc::new(Drift(1.0)));
        registry.register("mymod:b".into(), Arc::new(Drift(2.0)));
        registry.register("mymod:a".into(), Arc::new(Drift(3.0)));
        let providers = registry.into_providers();
        assert_eq!(providers.len(), 2);
        let mut a = particle_batch_from_vec(vec![particle()]);
        let mut b = particle_batch_from_vec(vec![particle()]);
        providers["mymod:a"].tick(&mut a, &Air);
        providers["mymod:b"].tick(&mut b, &Air);
        let at = |batch: &mut ParticleBatch| batch.get_mut(0).unwrap().position[0];
        assert_eq!((at(&mut a), at(&mut b)), (3.0, 2.0));
    }
}
