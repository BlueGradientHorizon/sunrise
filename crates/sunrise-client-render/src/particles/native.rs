//! Opt-in, compiled particle behavior. The ordinary engine still owns the
//! particle, its sprite set, interpolation, rendering and total count limits.
use super::{
    Behaviour, Particle, ParticleEngine, ParticleLevel, ParticleOptions, ParticleType, Sprite,
};
use mod_api::{
    __private::game::{CallbackOwner, ParticleProviderRegistration},
    Answered, BlockState, Identifier, LightLayer, ModParticle, ParticleFacing, ParticleLevelView,
    ParticleProvider, ParticleSpriteSet,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{collections::BTreeMap, fmt::Debug, sync::Arc, sync::OnceLock};
use sunrise_core::random::{LegacyRandom, RandomSource};

/// Requests made by one particle tick, submitted through normal spawn limits.
#[derive(Default)]
pub struct ParticleUpdate {
    /// An index in this particle's declared sprite set. Out-of-range is clamped.
    pub frame: Option<usize>,
    pub spawned: Vec<(ParticleOptions, [f64; 3], [f64; 3])>,
}

/// Owned behavior for a compiled native particle. Cloning a particle clones its
/// controller state. Do not retain world references; inspect `level` each tick.
pub trait ParticleController: Clone + Debug + Send + Sync + 'static {
    /// Call `particle.advance_age()` for ordinary lifetime/interpolation, or
    /// explicitly maintain those fields for an overridden source tick.
    fn tick(&mut self, particle: &mut Particle, level: &ParticleLevel<'_>) -> ParticleUpdate;
}

pub(super) trait Controller: Debug + Send + Sync {
    fn tick(&mut self, particle: &mut Particle, level: &ParticleLevel<'_>) -> ParticleUpdate;
}
impl<T: ParticleController> Controller for T {
    fn tick(&mut self, particle: &mut Particle, level: &ParticleLevel<'_>) -> ParticleUpdate {
        ParticleController::tick(self, particle, level)
    }
}

pub(super) type Factories = BTreeMap<String, Factory>;

/// Frozen before the first resource load. Each engine shares these factories, not their providers.
static REGISTERED: OnceLock<Arc<Factories>> = OnceLock::new();

/// Freeze attributed factories once at client startup. The last registration of an ID wins.
/// Returns false if the registry was already frozen.
pub fn register_particle_providers(providers: Vec<Answered<ParticleProviderRegistration>>) -> bool {
    if let Err(rejected) = REGISTERED.set(Arc::new(freeze(providers))) {
        for (id, factory) in rejected.iter() {
            log::warn!(
                "Particle provider {id} from mod {} was not registered: factories already frozen",
                factory.mod_id(),
            );
        }
        return false;
    }
    true
}

pub(super) fn registered() -> Arc<Factories> {
    REGISTERED.get().cloned().unwrap_or_default()
}

pub(super) fn freeze(providers: Vec<Answered<ParticleProviderRegistration>>) -> Factories {
    let mut factories = BTreeMap::new();
    for Answered { answer, mod_id, .. } in providers {
        let Some(owner) = CallbackOwner::new(mod_id) else {
            log::warn!(
                "Particle provider {} has no installed owner {mod_id}",
                answer.id
            );
            CallbackOwner::for_mod(mod_id).dispose(
                "particle provider",
                "drop_particle_provider_factory",
                answer,
            );
            continue;
        };
        let key = answer.id.to_string();
        if let Some(previous) = factories.insert(
            key,
            Factory {
                registration: Some(answer),
                owner,
            },
        ) {
            let registration = previous.registration.as_ref().expect("frozen factory");
            log::warn!(
                "Particle provider {} from mod {} was replaced by mod {mod_id}",
                registration.id,
                previous.owner.id(),
            );
        }
    }
    factories
}

pub(super) struct Factory {
    registration: Option<ParticleProviderRegistration>,
    owner: CallbackOwner,
}

impl Factory {
    pub(super) fn bind(&self, slot: usize, sprites: ParticleSpriteSet) -> Binding {
        let registration = self.registration.as_ref().expect("frozen factory");
        match self
            .owner
            .call("particle provider", "bind_particle_provider", || {
                BoundProvider {
                    id: registration.id.clone(),
                    owner: self.owner.clone(),
                    tick: Some((registration.factory)(sprites)),
                }
            }) {
            Ok(provider) => Binding::Bound(slot, Arc::new(provider)),
            Err(error) => {
                log::warn!("Particle provider {}: {error}", registration.id);
                Binding::Unavailable
            }
        }
    }

    pub(super) fn mod_id(&self) -> &'static str {
        self.owner.id()
    }
}

impl Drop for Factory {
    fn drop(&mut self) {
        self.owner.dispose(
            "particle provider",
            "drop_particle_provider_factory",
            self.registration.take(),
        );
    }
}

#[derive(Clone)]
pub(super) enum Binding {
    Declarative,
    Unavailable,
    Bound(usize, Arc<BoundProvider>),
}

pub(super) struct BoundProvider {
    id: Identifier,
    owner: CallbackOwner,
    tick: Option<Box<dyn ParticleProvider>>,
}

impl Drop for BoundProvider {
    fn drop(&mut self) {
        self.owner.dispose(
            "particle provider",
            "drop_particle_provider",
            self.tick.take(),
        );
    }
}

/// How far from the origin a provider may move a particle: the world border's limit. Past it, or
/// not a number, the particle expires rather than reach the renderer.
const LIMIT: f64 = 3.0e7;

/// How large a roll a provider may leave, in radians: half of `f32::MAX`, so the difference of
/// two accepted rolls, which the renderer interpolates, is a finite number. Past it, or not a
/// number, the particle expires.
const ROLL_LIMIT: f32 = f32::MAX / 2.0;

/// A particle of a mod's code-defined type. It owns the particle's behaviour for the engine: age
/// and expiry stay the engine's, the motion is the provider's, called once per type per tick with
/// every live particle of the type ([`Batch::run`]). Holds the provider the type resolved to when
/// its sprite sets loaded, so a tick dispatches straight to it.
pub(super) struct Provided {
    provider: Arc<BoundProvider>,
    /// The provider's dense slot among the registered ones, which its batch is filled by.
    slot: usize,
    /// `ModParticle::state`, kept from one tick to the next.
    state: [f64; 4],
    /// The frame of the sprite set drawn, as the provider last saw it.
    frame: u32,
    /// The engine aged the particle this tick and the provider has not run on it yet.
    pending: bool,
    /// How the quad turns toward the viewer, as the provider last set it.
    facing: Facing,
    /// The engine's count of live provider particles, which this one is part of until it drops.
    live: Arc<AtomicUsize>,
}
/// A provider particle's facing, as the renderer keeps it (`ParticleFacing`, the quaternion
/// already normalised).
#[derive(Clone, Copy, Debug)]
enum Facing {
    Camera,
    AroundY,
    Fixed(glam::Quat),
}
impl Clone for Provided {
    fn clone(&self) -> Self {
        self.live.fetch_add(1, Ordering::Relaxed);
        Self {
            provider: self.provider.clone(),
            slot: self.slot,
            state: self.state,
            frame: self.frame,
            pending: self.pending,
            facing: self.facing,
            live: self.live.clone(),
        }
    }
}
impl Drop for Provided {
    fn drop(&mut self) {
        self.live.fetch_sub(1, Ordering::Relaxed);
    }
}
impl Debug for Provided {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Provided")
            .field("state", &self.state)
            .field("frame", &self.frame)
            .finish_non_exhaustive()
    }
}
impl Provided {
    fn new(provider: Arc<BoundProvider>, slot: usize, frame: u32, live: &Arc<AtomicUsize>) -> Self {
        live.fetch_add(1, Ordering::Relaxed);
        Self {
            provider,
            slot,
            state: [0.0; 4],
            frame,
            pending: false,
            facing: Facing::Camera,
            live: live.clone(),
        }
    }
    /// The quad's rotation at `partial` of a tick (`SingleQuadParticle.extract`): the facing's
    /// quaternion (`LOOKAT_XYZ` the camera's, `LOOKAT_Y` its y and w alone, as vanilla sets it,
    /// or the world-fixed one), then `rotateZ` of `Mth.lerp(partial, oRoll, roll)` when the
    /// particle's roll is not zero. All of it in JOML's f32 maths.
    pub(super) fn rotation(
        &self,
        camera: glam::Quat,
        o_roll: f32,
        roll: f32,
        partial: f32,
    ) -> glam::Quat {
        use crate::camera::joml::Quat;
        let facing = match self.facing {
            Facing::Camera => Quat {
                x: camera.x,
                y: camera.y,
                z: camera.z,
                w: camera.w,
            },
            Facing::AroundY => Quat {
                x: 0.0,
                y: camera.y,
                z: 0.0,
                w: camera.w,
            },
            Facing::Fixed(q) => Quat {
                x: q.x,
                y: q.y,
                z: q.z,
                w: q.w,
            },
        };
        let turned = if roll == 0.0 {
            facing
        } else {
            facing.rotate_z(o_roll + partial * (roll - o_roll))
        };
        glam::Quat::from_xyzw(turned.x, turned.y, turned.z, turned.w)
    }
    /// The engine's part of the tick: age, `previous` and expiry. A particle that survives it
    /// joins this tick's batch.
    pub(super) fn age(&mut self, particle: &mut Particle) {
        self.pending = particle.advance_age();
    }
}

/// The batches of one tick, kept between ticks so native dispatch allocates nothing once warm.
/// One pass over the particles sorts them into a bucket per provider (by the dense slot the
/// provider was given when it registered); each provider then runs once on its bucket.
#[derive(Default)]
pub(super) struct Batch {
    /// The particle indices of each provider slot's batch.
    buckets: Vec<Vec<usize>>,
    /// The slots whose bucket holds a particle this pass, in the order they were first met.
    active: Vec<usize>,
    views: Vec<ModParticle>,
}
impl Batch {
    /// Sorts the particles the engine aged this tick into a bucket per provider slot. Whether
    /// any provider has a particle to tick.
    pub(super) fn gather(&mut self, particles: &[Particle]) -> bool {
        for slot in self.active.drain(..) {
            self.buckets[slot].clear();
        }
        for (i, p) in particles.iter().enumerate() {
            if let Some(provided) = p.provided.as_ref().filter(|q| q.pending) {
                let slot = provided.slot;
                if self.buckets.len() <= slot {
                    self.buckets.resize_with(slot + 1, Vec::new);
                }
                if self.buckets[slot].is_empty() {
                    self.active.push(slot);
                }
                self.buckets[slot].push(i);
            }
        }
        !self.active.is_empty()
    }
    /// Calls each provider once with the particles [`Batch::gather`] found for it, then takes the
    /// results back: a particle that leaves a number past the world's limit or not finite
    /// expires.
    pub(super) fn run(
        &mut self,
        particles: &mut [Particle],
        level: &dyn ParticleLevelView,
        mut set_frame: impl FnMut(&mut Particle, usize),
    ) {
        for &slot in &self.active {
            let bucket = &self.buckets[slot];
            let provider = particles[bucket[0]]
                .provided
                .as_ref()
                .map(|provided| provided.provider.clone())
                .expect("a bucket holds provider particles");
            self.views.clear();
            for &i in bucket {
                let p = &particles[i];
                let provided = p
                    .provided
                    .as_ref()
                    .expect("a bucket holds provider particles");
                self.views.push(ModParticle {
                    position: p.position,
                    previous: p.previous,
                    velocity: p.velocity,
                    age: p.age,
                    lifetime: p.lifetime,
                    size: p.size,
                    rgba: p.rgba,
                    frame: provided.frame,
                    removed: false,
                    state: provided.state,
                    facing: p.facing(),
                    roll: p.roll,
                });
            }
            let mut batch =
                mod_api::__private::particle_batch_from_vec(std::mem::take(&mut self.views));
            let ran = provider
                .owner
                .call("particle provider", "tick_particle_provider", || {
                    provider
                        .tick
                        .as_ref()
                        .expect("bound provider")
                        .tick(&mut batch, level);
                });
            self.views = mod_api::__private::particle_batch_into_vec(batch);
            if let Err(error) = ran {
                log::warn!("Particle provider {}: {error}", provider.id);
                for &i in bucket {
                    particles[i]
                        .provided
                        .as_mut()
                        .expect("a bucket holds provider particles")
                        .pending = false;
                }
                continue;
            }
            if self.views.len() != bucket.len() {
                // A guest handed back a batch of another size: nothing of it can be matched to a particle.
                for &i in bucket {
                    particles[i].remove();
                }
                continue;
            }
            for (view, &i) in self.views.iter().zip(bucket) {
                if let Some(frame) = particles[i].take(view) {
                    set_frame(&mut particles[i], frame);
                }
            }
        }
    }
}
impl Particle {
    /// The facing the provider sees.
    fn facing(&self) -> ParticleFacing {
        match self.provided.as_ref().map(|q| q.facing) {
            Some(Facing::Fixed(q)) => ParticleFacing::Fixed(q.to_array()),
            Some(Facing::AroundY) => ParticleFacing::AroundY,
            _ => ParticleFacing::Camera,
        }
    }
    /// What a provider's tick left of the particle, validated as the renderer needs it.
    fn take(&mut self, view: &ModParticle) -> Option<usize> {
        let finite = |v: &[f64; 3]| v.iter().all(|c| c.abs() <= LIMIT);
        let facing = match view.facing {
            ParticleFacing::Fixed(q) => {
                let q = glam::Quat::from_array(q);
                let length_squared = q.length_squared();
                (length_squared.is_finite() && length_squared > 0.0)
                    .then(|| Facing::Fixed(q.normalize()))
            }
            ParticleFacing::AroundY => Some(Facing::AroundY),
            ParticleFacing::Camera => Some(Facing::Camera),
        };
        let provided = self.provided.as_mut()?;
        provided.pending = false;
        let Some(facing) = facing else {
            self.remove();
            return None;
        };
        if view.removed
            || !finite(&view.position)
            || !finite(&view.previous)
            || !finite(&view.velocity)
            || !view.size.is_finite()
            || !view.roll.is_finite()
            || view.roll.abs() > ROLL_LIMIT
            || !view.rgba.iter().all(|c| c.is_finite())
        {
            self.remove();
            return None;
        }
        self.position = view.position;
        self.previous = view.previous;
        self.velocity = view.velocity;
        self.lifetime = view.lifetime;
        // The declarative range of a mod particle's size (`sunrise:behaviour`) and of its colour.
        self.size = view.size.clamp(0.0, 16.0);
        self.rgba = view.rgba.map(|c| c.clamp(0.0, 1.0));
        self.roll = view.roll;
        provided.facing = facing;
        provided.state = view.state;
        let changed = view.frame != provided.frame;
        provided.frame = view.frame;
        changed.then_some(view.frame as usize)
    }
}

/// `ParticleLevelView` over the engine's level: the loaded chunks' states and stored light.
pub(super) struct ProviderLevel<'a, 'l> {
    pub(super) level: &'a ParticleLevel<'l>,
    /// Every engine state as a mod reads it, by state id.
    pub(super) states: &'a [BlockState],
}
impl ParticleLevelView for ProviderLevel<'_, '_> {
    fn block_state(&self, [x, y, z]: [i32; 3]) -> BlockState {
        // The table holds every state the world can: an id outside it is the game's bug.
        self.states[self.level.block(x, y, z).0 as usize]
    }
    fn brightness(&self, layer: LightLayer, [x, y, z]: [i32; 3]) -> u8 {
        let world = self.level.world;
        match layer {
            LightLayer::Block => world.light(x, y, z) & 15,
            // A dimension without skylight has no sky layer to read.
            LightLayer::Sky if !world.shape.sky_light => 0,
            LightLayer::Sky => world.light(x, y, z) >> 4,
        }
    }
}

impl ParticleEngine {
    pub(super) fn make_named(
        &mut self,
        name: &str,
        p: [f64; 3],
        v: [f64; 3],
        seed: i64,
    ) -> Option<Particle> {
        // A native mod's SimpleParticleType: vanilla's TextureSheetParticle with
        // the sprite set its particles/<name>.json names, no tint, 20..40 ticks.
        let entry = self.mod_sets.get(name)?;
        let (set, behaviour, group) = (entry.set, entry.behaviour, entry.group);
        let provider = match &entry.provider {
            Binding::Declarative => None,
            Binding::Unavailable => return None,
            Binding::Bound(slot, provider) => {
                if !provider.owner.is_enabled() {
                    log::warn!(
                        "Particle provider {name}: mod {} callback is disabled",
                        provider.owner.id()
                    );
                    return None;
                }
                Some((*slot, Arc::clone(provider)))
            }
        };
        let mut r = LegacyRandom::new(seed);
        let mut q = Particle::moving(&mut r, ParticleType::Smoke, p, v);
        q.ty = ParticleType::Smoke;
        q.gravity = 0.0;
        q.rgba = [1.0; 4];
        q.lifetime = 20 + r.next_int_bound(20).unwrap_or(0);
        q.set = Some(set);
        q.group = group;
        let sprites = &self.set_sprites[set as usize];
        q.sprite =
            Sprite::Index(sprites[r.next_int_bound(sprites.len() as i32).unwrap_or(0) as usize]);
        if let Some(behaviour) = behaviour {
            behaviour.apply(&mut q, v, &mut r);
            if behaviour.starts_on_first_sprite() {
                q.sprite = Sprite::Index(sprites[0]);
            }
        }
        if let Some((slot, provider)) = provider {
            // The declared values seed the particle; the provider owns its tick from here.
            // Its sprite is chosen by frame, so the set stays attached.
            let frame = sprites
                .iter()
                .position(|&s| matches!(q.sprite, Sprite::Index(i) if i == s))
                .unwrap_or(0);
            q.set = Some(set);
            q.behaviour = Behaviour::Controlled { translucent: true };
            q.provided = Some(Box::new(Provided::new(
                provider,
                slot,
                frame as u32,
                &self.provided_live,
            )));
        }
        Some(q)
    }

    /// The providers' part of a tick: each provider once with every particle of its type that
    /// `tick_particle` aged. Nothing runs while no provider particle lives.
    pub(super) fn tick_providers(&mut self, particles: &mut [Particle], level: &ParticleLevel) {
        if self
            .provided_live
            .load(std::sync::atomic::Ordering::Relaxed)
            == 0
        {
            return;
        }
        let mut batch = std::mem::take(&mut self.batch);
        if batch.gather(particles)
            && let Some(states) = (self.block_state_source)()
        {
            let view = ProviderLevel {
                level,
                states: (*states).as_ref(),
            };
            batch.run(particles, &view, |p, frame| self.set_sprite_frame(p, frame));
        }
        self.batch = batch;
    }
}

#[cfg(test)]
pub(super) mod tests;
