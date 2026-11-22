//! Code-defined particle providers, as a client mod registers them in `register_particle_providers`
//! (NeoForge's `RegisterParticleProvidersEvent` and `ParticleProvider`).
//!
//! A provider owns one of the mod's own particle types, named by its ID (`mymod:spark`) and spawned
//! with `ParticleOptions::simple` on the key `Registrar::particle_key` issued for it. Its factory
//! binds once per sprite load, after the game's sprite discovery. The sprite set and the
//! `sunrise:behaviour` of `assets/<namespace>/particles/<path>.json` give the spawn values; the
//! bound provider then moves the particle every tick.
use crate::{BlockState, Identifier, LightLayer, RegisterParticleProvidersEvent};

/// One particle of a provider's type, as its tick reads and writes it: the fields of vanilla's
/// `Particle` a `tick` moves. Plain numbers, so a tick allocates nothing.
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
    /// interpolates it from the value the previous tick left, as vanilla's `oRoll` does, and
    /// draws no turn at all while it is zero. A roll beyond half of `f32::MAX` in either
    /// direction expires the particle, so that the interpolation stays a finite number.
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
    /// `LOOKAT_Y`: the camera's rotation reduced to its Y turn, as vanilla sets it: the
    /// quaternion made of the camera's `y` and `w` components alone. The particle's own position
    /// does not enter.
    AroundY,
    /// A world-fixed orientation, as a unit quaternion `[x, y, z, w]` turning the quad's
    /// default plane (corners in XY, facing +Z). A quad lying flat facing up is
    /// `[-sin(pi/4), 0, 0, cos(pi/4)]`. `roll` turns the quad about its own normal after it.
    Fixed([f32; 4]),
}

/// The particles of one provider's batch. Edit them through [`ParticleBatch::get_mut`],
/// [`ParticleBatch::iter_mut`] or a `for` loop: those methods preserve the batch's length and
/// order. Set [`ModParticle::removed`] to expire a particle. The helpers that build the batch
/// and take it back are internal. If a callback returns a different length, the game expires
/// the batch's particles.
///
/// Dispatch reuses its batch storage once it has enough capacity.
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
/// Internal constructor: wrap the game's `particles` for a provider tick.
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
/// `Particle` code reads it (`level.getBlockState`, `level.getBrightness`). Read-only. `block_state`
/// reads air where a chunk is absent. `brightness` reads the render level's stored light, with sky
/// always zero in a dimension without skylight. The current client gives absent columns and
/// positions above build height full sky where present and no block light; below a loaded column
/// it gives zero on both layers. Retained and padding light are tracked in Sunrise #2761.
pub trait ParticleLevelView {
    /// `BlockGetter.getBlockState`: the state at `pos`.
    fn block_state(&self, pos: [i32; 3]) -> BlockState;
    /// `BlockAndLightGetter.getBrightness`: the stored light of one layer at `pos`, 0 to 15.
    fn brightness(&self, layer: LightLayer, pos: [i32; 3]) -> u8;
}

/// A mod's particle provider: NeoForge's `ParticleProvider`, created by a factory registered with
/// [`RegisterParticleProvidersEvent::register_sprite_set`]. Particles of a type share the bound
/// provider, so per-particle data lives in [`ModParticle::state`], not in the provider.
///
/// A tick has no `Host`: the engine ticks particles inside the render crate, which reaches no
/// gameplay. It does read the level through [`ParticleLevelView`].
pub trait ParticleProvider: Send + Sync + 'static {
    /// `Particle.tick` for every live particle of this provider's type: one call per type per
    /// tick, with the whole batch. The game has set `previous` and `age`
    /// on each already and expires a particle once `age` passes `lifetime`; the tick sets
    /// `position`, `frame`, `size`, `rgba`, `lifetime`, `removed`, `facing` and `roll`. A number
    /// that is not finite expires the particle. Batch iteration preserves length and order.
    /// Set `removed` to expire a particle. If a callback returns a different length, the engine
    /// expires the batch's particles. Not called when no particle of the type is alive.
    /// A failed call does not apply the batch. A panic disables this provider registration for
    /// the session and is reported with its mod in the log and the client error toast.
    fn tick(&self, particles: &mut ParticleBatch, level: &dyn ParticleLevelView);
}

/// The nonempty sprite set bound for a provider on this resource load. It describes the frames
/// from the particle's JSON, in declaration order, without exposing atlas data or engine handles.
// no wire: describes a sprite set bound for the current in-process resource load.
// exhaustive: the game constructs this descriptor; the field is private.
#[derive(Clone, Copy)]
pub struct ParticleSpriteSet {
    frame_count: usize,
}

impl ParticleSpriteSet {
    /// The number of frames in the bound set. Frame indices start at zero.
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }
}

/// Describe a sprite set the renderer has already loaded and found to be nonempty.
pub fn particle_sprite_set(frame_count: usize) -> ParticleSpriteSet {
    ParticleSpriteSet { frame_count }
}

impl RegisterParticleProvidersEvent<'_> {
    /// Stage a factory for a particle type's `assets/<namespace>/particles/<path>.json` sprite set
    /// (NeoForge's `registerSpriteSet`). Use the ID passed to `Registrar::particle_key`.
    /// A phase-refused listener's entries are discarded. For each ID, only the last accepted
    /// factory runs, once per load with that load's nonempty set. A missing set or failed bind
    /// is reported and no particles of the type are spawned, rather than using declarative motion.
    pub fn register_sprite_set<P: ParticleProvider>(
        &mut self,
        id: Identifier,
        factory: impl Fn(ParticleSpriteSet) -> P + Send + Sync + 'static,
    ) {
        self.providers.push(ParticleProviderRegistration {
            id,
            factory: Box::new(move |sprites| Box::new(factory(sprites))),
        });
    }
}

/// A staged sprite-set factory; dispatch attributes it to the registering mod.
// no wire: registration retains executable native code.
#[non_exhaustive]
pub struct ParticleProviderRegistration {
    /// The registered particle type's ID.
    pub id: Identifier,
    /// Constructs the provider for the loaded nonempty sprite set.
    pub factory: Box<dyn Fn(ParticleSpriteSet) -> Box<dyn ParticleProvider> + Send + Sync>,
}
