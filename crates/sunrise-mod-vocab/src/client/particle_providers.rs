//! Code-defined particle providers, as a client mod registers them in `register_particle_providers`
//! (NeoForge's `RegisterParticleProvidersEvent` and `ParticleProvider`).
//!
//! A sprite-set factory binds once per resource load. The bound provider constructs an owned
//! particle for each admitted spawn request, before the particle engine considers its group quota.
use crate::{
    BlockState, Identifier, LightLayer, ParticleOptions, RandomSource,
    RegisterParticleProvidersEvent,
};

/// How a provider's particle quad is oriented (`SingleQuadParticle.FacingCameraMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
// exhaustive: a closed set the game matches without a wildcard
// no wire: presentation state for a live local particle, not a spawn option.
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

/// What a provider or particle reads of the level around its particles: the loaded world as
/// `Particle` code reads it (`level.getBlockState`, `level.getBrightness`). Read-only. `block_state`
/// reads air where a chunk is absent. `brightness` reads the render level's stored light, with sky
/// always zero in a dimension without skylight. The current client gives absent columns and
/// positions above build height full sky where present and no block light; below a loaded column
/// it gives zero on both layers. The loan lasts only for the current callback.
// no wire: reads the loaded level in process.
pub trait ParticleLevelView {
    /// `BlockGetter.getBlockState`: the state at `pos`.
    fn block_state(&self, pos: [i32; 3]) -> BlockState;
    /// `BlockAndLightGetter.getBrightness`: the stored light of one layer at `pos`, 0 to 15.
    fn brightness(&self, layer: LightLayer, pos: [i32; 3]) -> u8;
}

/// Inputs borrowed on the particle engine's owning thread for one construction call.
/// Keep only owned data in the particle returned by the provider.
// no wire: holds live options, level and random loans for the duration of the call.
#[non_exhaustive]
pub struct ParticleSpawn<'a> {
    /// The registered request and its typed options, before construction.
    pub options: &'a ParticleOptions,
    /// The raw position the caller requested, in world coordinates.
    pub position: [f64; 3],
    /// The raw auxiliary velocity slots the caller supplied, before constructor defaults.
    pub velocity: [f64; 3],
    /// Read-only access to loaded block states and stored light.
    pub level: &'a dyn ParticleLevelView,
    /// The engine's provider random. Draws advance the live stream in call order.
    pub random: &'a mut dyn RandomSource,
}

/// Construct the factory's temporary loan of the live spawn inputs.
pub fn particle_spawn<'a>(
    options: &'a ParticleOptions,
    position: [f64; 3],
    velocity: [f64; 3],
    level: &'a dyn ParticleLevelView,
    random: &'a mut dyn RandomSource,
) -> ParticleSpawn<'a> {
    ParticleSpawn {
        options,
        position,
        velocity,
        level,
        random,
    }
}

/// Read-only access to one live particle, borrowed for the current callback only.
// no wire: reads the engine-owned particle directly, not a snapshot.
pub trait ParticleView {
    /// The current position, in world coordinates.
    fn position(&self) -> [f64; 3];
    /// The previous tick's position, which rendering interpolates from.
    fn previous_position(&self) -> [f64; 3];
    /// The current velocity in blocks per tick.
    fn velocity(&self) -> [f64; 3];
    /// `Particle.age`: zero at construction, advanced by the particle's tick.
    fn age(&self) -> i32;
    /// `Particle.lifetime`: the base tick removes the particle when its pre-increment age reaches this.
    fn lifetime(&self) -> i32;
    /// Whether the latest collision step hit the ground.
    fn on_ground(&self) -> bool;
    /// Whether the particle has not been removed.
    fn is_alive(&self) -> bool;
    /// `SingleQuadParticle.quadSize`: the billboard half-width in blocks.
    fn size(&self) -> f32;
    /// Red, green, blue and alpha of the quad.
    fn rgba(&self) -> [f32; 4];
    /// The selected zero-based index in the bound sprite set.
    fn sprite_index(&self) -> usize;
    /// How the quad faces the viewer or world.
    fn facing(&self) -> ParticleFacing;
    /// `SingleQuadParticle.roll`: rotation about the quad normal, in radians.
    fn roll(&self) -> f32;
    /// The prior roll that rendering interpolates from if the current roll is nonzero.
    fn previous_roll(&self) -> f32;
}

/// Authoritative access to one particle during its construction or tick on the owning thread.
/// A write takes effect immediately. Invalid setter inputs are reported with the mod's name
/// and return `false` without changing the field or killing the particle. Do not retain this view.
// no wire: mutates a live particle and borrows its level and sprite set in process.
pub trait ParticleTickView: ParticleView {
    /// `Particle.setPos`: set the position and rebuild its collision box.
    fn set_position(&mut self, position: [f64; 3]) -> bool;
    /// Set the interpolation origin, for example to snap to a teleported position.
    fn set_previous_position(&mut self, position: [f64; 3]) -> bool;
    /// `Particle.setParticleSpeed`: replace the velocity, without moving yet.
    fn set_velocity(&mut self, velocity: [f64; 3]) -> bool;
    /// Set the current age in ticks.
    fn set_age(&mut self, age: i32);
    /// `Particle.setLifetime`: set the age at which the base tick will remove this particle.
    fn set_lifetime(&mut self, lifetime: i32);
    /// Set the gravity coefficient applied by the base tick.
    fn set_gravity(&mut self, gravity: f32) -> bool;
    /// Set the velocity multiplier applied by the base tick after movement.
    fn set_friction(&mut self, friction: f32) -> bool;
    /// Whether to collide with blocks when moving.
    fn set_has_physics(&mut self, has_physics: bool);
    /// Set the billboard half-width, independent of the physics box.
    fn set_size(&mut self, size: f32) -> bool;
    /// Set red, green, blue and alpha without clamping them.
    fn set_rgba(&mut self, rgba: [f32; 4]) -> bool;
    /// Select an exact zero-based sprite index. An absent index is reported, not clamped.
    fn set_sprite_index(&mut self, index: usize) -> bool;
    /// `SingleQuadParticle.setSpriteFromAge`: select `age * (frame_count - 1) / lifetime`
    /// with Java integer arithmetic. A removed particle is left unchanged; zero lifetime
    /// or a missing index is reported. Returns whether a sprite was selected.
    fn set_sprite_from_age(&mut self) -> bool;
    /// Set the quad facing. A fixed quaternion must be finite and nonzero and is normalized.
    fn set_facing(&mut self, facing: ParticleFacing) -> bool;
    /// Set rotation about the quad normal, in radians.
    fn set_roll(&mut self, roll: f32) -> bool;
    /// `Particle.move`: apply a displacement using the particle's collision box and live level.
    fn move_by(&mut self, displacement: [f64; 3]) -> bool;
    /// Record the previous position and roll, increment age, and remove at the same pre-increment
    /// lifetime check as `Particle.tick`. Returns false on expiry. Does not move the particle.
    fn advance_age(&mut self) -> bool;
    /// Run `Particle.tick`: capture the previous pose, check lifetime, then apply gravity,
    /// movement and friction. Sprite selection is separate.
    fn tick_base(&mut self);
    /// `Particle.remove`: mark this particle to be retired by the engine.
    fn remove(&mut self);
}

/// An owned native particle instance. Its fields are its private per-particle state. It is
/// constructed, ticked and destroyed by the particle engine, not copied to a worker or a batch.
// no wire: owns executable behavior and private instance state.
pub trait CustomParticle: Send + Sync + 'static {
    /// Tick this instance on the owning thread, with particle and level loans for this call.
    /// The default runs the base tick. Overrides may call `tick_base` or use `advance_age`
    /// and move themselves. A panic disables this registration's tick callback for the session
    /// and retires its instances without disabling other registrations.
    fn tick(&mut self, particle: &mut dyn ParticleTickView, _level: &dyn ParticleLevelView) {
        particle.tick_base();
    }
}

/// A mod's particle provider, bound to a sprite set by
/// [`RegisterParticleProvidersEvent::register_sprite_set`]. The provider is shared by that
/// type's spawns; the `CustomParticle` it constructs is not shared.
// no wire: retained native factory called with scoped level and particle loans.
pub trait ParticleProvider: Send + Sync + 'static {
    /// `ParticleProvider.createParticle`: construct one instance synchronously with the spawn
    /// inputs and a mutable view seeded with the particle's constructor defaults. Return `None`
    /// to decline this spawn. A returned particle joins the normal pending queue and may still
    /// be rejected by group admission. A panic disables this registration's create callback for
    /// the session and is reported with the mod's name in the log and client error toast.
    fn create_particle(
        &self,
        spawn: &mut ParticleSpawn<'_>,
        particle: &mut dyn ParticleTickView,
    ) -> Option<Box<dyn CustomParticle>>;
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
