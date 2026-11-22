//! Opt-in, compiled particle behavior. The ordinary engine still owns the
//! particle, its sprite set, interpolation, rendering and total count limits.
use super::{
    Behaviour, Particle, ParticleEngine, ParticleLevel, ParticleOptions, ParticleType,
    QUAD_CORNERS, SpawnScope, Sprite,
};
use glam::Vec3;
use mod_api::{
    __private::game::{CallbackOwner, ParticleProviderRegistration, particle_spawn},
    Answered, BlockState, CustomParticle, Identifier, LightLayer, ParticleFacing,
    ParticleLevelView, ParticleProvider, ParticleQuad, ParticleSpriteSet, ParticleTickView,
    ParticleView,
};
use std::{
    collections::BTreeMap,
    fmt::Debug,
    ops::Deref,
    sync::{Arc, OnceLock},
};
use sunrise_core::random::{LegacyRandom, RandomSource};

/// Requests made by one particle tick, submitted through normal spawn limits.
#[derive(Default)]
pub struct ParticleUpdate {
    /// An index in this particle's declared sprite set. Out-of-range is clamped.
    pub frame: Option<usize>,
    pub spawned: Vec<(ParticleOptions, [f64; 3], [f64; 3])>,
}

/// Engine-side controller for a compiled particle, separate from mod providers.
/// Do not retain world references; inspect `level` each tick.
pub trait ParticleController: Debug + Send + Sync + 'static {
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
        let callbacks = Arc::new(ProviderCallbacks {
            id: answer.id.clone(),
            create: CallbackOwner::for_mod(mod_id),
            tick: CallbackOwner::for_mod(mod_id),
            extract: CallbackOwner::for_mod(mod_id),
            dispose: CallbackOwner::for_mod(mod_id),
        });
        if let Some(previous) = factories.insert(
            key,
            Factory {
                registration: Some(answer),
                owner,
                callbacks,
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
    callbacks: Arc<ProviderCallbacks>,
}

impl Factory {
    pub(super) fn bind(&self, sprites: ParticleSpriteSet) -> Binding {
        let registration = self.registration.as_ref().expect("frozen factory");
        let mut binding = BoundProvider {
            callbacks: Arc::clone(&self.callbacks),
            provider: None,
        };
        // If a factory catches its own phase unwind, a returned provider is still disposed
        // under its owner instead of being dropped by the refusal boundary.
        match self
            .owner
            .call("particle provider", "bind_particle_provider", || {
                binding.provider = Some((registration.factory)(sprites));
            }) {
            Ok(()) => Binding::Bound(binding),
            Err(error) => {
                log::warn!("Particle provider {}: {error}", registration.id);
                Binding::Unavailable {
                    mod_id: self.mod_id(),
                }
            }
        }
    }

    pub(super) fn mod_id(&self) -> &'static str {
        self.owner.id()
    }
}

impl Drop for Factory {
    fn drop(&mut self) {
        self.callbacks.dispose.dispose(
            "particle provider",
            "drop_particle_provider_factory",
            self.registration.take(),
        );
    }
}

pub(super) enum Binding {
    Declarative,
    Unavailable { mod_id: &'static str },
    Bound(BoundProvider),
}

/// Set up once for the winning registration and shared across loads and instances.
struct ProviderCallbacks {
    id: Identifier,
    create: CallbackOwner,
    tick: CallbackOwner,
    extract: CallbackOwner,
    dispose: CallbackOwner,
}

pub(super) struct BoundProvider {
    callbacks: Arc<ProviderCallbacks>,
    provider: Option<Box<dyn ParticleProvider>>,
}

impl Drop for BoundProvider {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            self.callbacks
                .dispose
                .dispose("particle provider", "drop_particle_provider", provider);
        }
    }
}

/// One native instance and the quad presentation state initialized with its sprite set.
pub(super) struct Provided {
    callbacks: Arc<ProviderCallbacks>,
    instance: Option<Box<dyn CustomParticle>>,
    presentation: Presentation,
}

struct Presentation {
    frame: usize,
    facing: ParticleFacing,
}

pub(super) struct ExtractedQuad {
    pub(super) corners: [Vec3; 4],
    pub(super) size: f32,
    pub(super) color: u32,
}

impl Drop for Provided {
    fn drop(&mut self) {
        if let Some(instance) = self.instance.take() {
            self.callbacks
                .dispose
                .dispose("particle provider", "drop_custom_particle", instance);
        }
    }
}

impl Debug for Provided {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Provided")
            .field("id", &self.callbacks.id)
            .field("frame", &self.presentation.frame)
            .finish_non_exhaustive()
    }
}

impl Provided {
    pub(super) fn tick(
        &mut self,
        engine: &ParticleEngine,
        particle: &mut Particle,
        level: &ParticleLevel<'_>,
    ) {
        let owner = &self.callbacks.tick;
        // A sibling instance may already have poisoned the shared callback.
        if owner.is_poisoned() {
            particle.remove();
            return;
        }
        let Some(states) = (engine.block_state_source)() else {
            log::warn!(
                "Particle provider {} from mod {}: no block states for tick",
                self.callbacks.id,
                owner.id()
            );
            return;
        };
        let level = ProviderLevel {
            level,
            states: (*states).as_ref(),
        };
        let set = particle.set.expect("provided particle has a sprite set");
        let mut view = LiveParticle {
            particle: &mut *particle,
            presentation: &mut self.presentation,
            context: TickContext {
                level: level.level,
                sprites: &engine.set_sprites[set as usize],
                callbacks: &self.callbacks,
            },
        };
        if let Err(error) = owner.call("particle provider", "tick_custom_particle", || {
            self.instance
                .as_mut()
                .expect("owned particle")
                .tick(&mut view, &level);
        }) {
            log::warn!("Particle provider {}: {error}", self.callbacks.id);
            if owner.is_poisoned() {
                particle.remove();
            }
        }
    }
    /// Run the frame callback and prepare a complete quad before submitting any vertices.
    pub(super) fn extract(
        &self,
        particle: &Particle,
        camera: glam::Quat,
        center: Vec3,
        partial: f32,
    ) -> Option<ExtractedQuad> {
        use crate::camera::joml::Quat;
        let owner = &self.callbacks.extract;
        if owner.is_poisoned() {
            return None;
        }
        let view = LiveParticle {
            particle,
            presentation: &self.presentation,
            context: (),
        };
        let mut quad = ParticleQuad {
            size: particle.quad_size(partial),
            rgba: view.rgba(),
            facing: view.facing(),
            roll: view.roll(),
            visible: true,
        };
        if let Err(error) = owner.call("particle provider", "extract_custom_particle", || {
            self.instance
                .as_ref()
                .expect("owned particle")
                .extract(&view, partial, &mut quad);
        }) {
            log::warn!("Particle provider {}: {error}", self.callbacks.id);
            return None;
        }
        if !quad.visible {
            return None;
        }

        // SingleQuadParticle.extract: keep the raw LOOKAT_Y y/w (or the fixed rotation),
        // and apply JOML rotateZ only if the current roll is nonzero.
        let [x, y, z, w] = match quad.facing {
            ParticleFacing::Camera => camera.to_array(),
            ParticleFacing::AroundY => [0.0, camera.y, 0.0, camera.w],
            ParticleFacing::Fixed(q) => q,
        };
        let mut rotation = Quat { x, y, z, w };
        if quad.roll != 0.0 {
            let previous = view.previous_roll();
            rotation = rotation.rotate_z(previous + partial * (quad.roll - previous));
        }
        let corners =
            QUAD_CORNERS.map(|corner| center + Vec3::from(rotation.transform(corner)) * quad.size);
        if corners.iter().any(|corner| !corner.is_finite()) {
            log::warn!(
                "Particle provider {} from mod {}: non-finite quad vertices",
                self.callbacks.id,
                owner.id(),
            );
            return None;
        }

        // ARGB.colorFromFloat masks each floored channel to one byte. The renderer's
        // Unorm8x4 vertex attribute consumes those bytes in RGBA order.
        let rgba = quad
            .rgba
            .map(|channel| sunrise_core::mth::floor_f32(channel * 255.0) as u8);
        Some(ExtractedQuad {
            corners,
            size: quad.size,
            color: u32::from_le_bytes(rgba),
        })
    }
}

// Read and write callbacks share the same getters. Only the tick/create loan carries
// mutable state and the level/sprite context for ParticleTickView.
struct LiveParticle<P, S, C> {
    particle: P,
    presentation: S,
    context: C,
}

struct TickContext<'a, 'l> {
    level: &'a ParticleLevel<'l>,
    sprites: &'a [u32],
    callbacks: &'a ProviderCallbacks,
}

type TickParticle<'a, 'l> =
    LiveParticle<&'a mut Particle, &'a mut Presentation, TickContext<'a, 'l>>;

impl TickParticle<'_, '_> {
    #[cold]
    fn reject(&self, input: &'static str) -> bool {
        let callbacks = self.context.callbacks;
        log::warn!(
            "Particle provider {} from mod {}: invalid {input}",
            callbacks.id,
            callbacks.tick.id(),
        );
        false
    }
}

impl<P: Deref<Target = Particle>, S: Deref<Target = Presentation>, C> ParticleView
    for LiveParticle<P, S, C>
{
    fn position(&self) -> [f64; 3] {
        self.particle.position
    }
    fn previous_position(&self) -> [f64; 3] {
        self.particle.previous
    }
    fn velocity(&self) -> [f64; 3] {
        self.particle.velocity
    }
    fn age(&self) -> i32 {
        self.particle.age
    }
    fn lifetime(&self) -> i32 {
        self.particle.lifetime
    }
    fn on_ground(&self) -> bool {
        self.particle.on_ground
    }
    fn is_alive(&self) -> bool {
        self.particle.is_alive()
    }
    fn size(&self) -> f32 {
        self.particle.size
    }
    fn rgba(&self) -> [f32; 4] {
        self.particle.rgba
    }
    fn sprite_index(&self) -> usize {
        self.presentation.frame
    }
    fn facing(&self) -> ParticleFacing {
        self.presentation.facing
    }
    fn roll(&self) -> f32 {
        self.particle.roll
    }
    fn previous_roll(&self) -> f32 {
        self.particle.o_roll
    }
}

impl ParticleTickView for TickParticle<'_, '_> {
    fn set_position(&mut self, position: [f64; 3]) -> bool {
        if !position.into_iter().all(f64::is_finite) {
            return self.reject("position");
        }
        self.particle.set_pos(position);
        true
    }

    fn set_previous_position(&mut self, position: [f64; 3]) -> bool {
        if !position.into_iter().all(f64::is_finite) {
            return self.reject("previous position");
        }
        self.particle.previous = position;
        true
    }

    fn set_velocity(&mut self, velocity: [f64; 3]) -> bool {
        if !velocity.into_iter().all(f64::is_finite) {
            return self.reject("velocity");
        }
        self.particle.velocity = velocity;
        true
    }

    fn set_age(&mut self, age: i32) {
        self.particle.age = age;
    }
    fn set_lifetime(&mut self, lifetime: i32) {
        self.particle.lifetime = lifetime;
    }
    fn set_has_physics(&mut self, has_physics: bool) {
        self.particle.has_physics = has_physics;
    }

    fn set_gravity(&mut self, gravity: f32) -> bool {
        if !gravity.is_finite() {
            return self.reject("gravity");
        }
        self.particle.gravity = gravity;
        true
    }

    fn set_friction(&mut self, friction: f32) -> bool {
        if !friction.is_finite() {
            return self.reject("friction");
        }
        self.particle.friction = friction;
        true
    }

    fn set_size(&mut self, size: f32) -> bool {
        if !size.is_finite() {
            return self.reject("quad size");
        }
        self.particle.size = size;
        true
    }

    fn set_rgba(&mut self, rgba: [f32; 4]) -> bool {
        if !rgba.into_iter().all(f32::is_finite) {
            return self.reject("quad color");
        }
        self.particle.rgba = rgba;
        true
    }

    fn set_sprite_index(&mut self, index: usize) -> bool {
        let Some(&sprite) = self.context.sprites.get(index) else {
            return self.reject("sprite index");
        };
        self.presentation.frame = index;
        self.particle.sprite = Sprite::Index(sprite);
        true
    }

    fn set_sprite_from_age(&mut self) -> bool {
        if !self.particle.is_alive() {
            return false;
        }
        let lifetime = self.particle.lifetime;
        if lifetime == 0 {
            return self.reject("zero lifetime for sprite selection");
        }
        let index = self
            .particle
            .age
            .wrapping_mul((self.context.sprites.len() - 1) as i32)
            .wrapping_div(lifetime);
        self.set_sprite_index(index as usize)
    }

    fn set_facing(&mut self, facing: ParticleFacing) -> bool {
        self.presentation.facing = if let ParticleFacing::Fixed(q) = facing {
            // Widen for the norm so every finite nonzero f32 quaternion can be normalized.
            let q = glam::DQuat::from_array(q.map(f64::from));
            let length = q.length();
            if !length.is_finite() || length == 0.0 {
                return self.reject("fixed facing");
            }
            ParticleFacing::Fixed(q.to_array().map(|value| (value / length) as f32))
        } else {
            facing
        };
        true
    }

    fn set_roll(&mut self, roll: f32) -> bool {
        if !roll.is_finite() {
            return self.reject("roll");
        }
        self.particle.roll = roll;
        true
    }

    fn move_by(&mut self, displacement: [f64; 3]) -> bool {
        if !displacement.into_iter().all(f64::is_finite) {
            return self.reject("displacement");
        }
        self.particle.move_by(self.context.level, displacement);
        true
    }

    fn advance_age(&mut self) -> bool {
        self.particle.advance_age()
    }

    fn tick_base(&mut self) {
        self.particle.o_roll = self.particle.roll;
        self.particle.base_tick(self.context.level);
    }

    fn remove(&mut self) {
        self.particle.remove();
    }
}

struct ProviderRandom<'a>(&'a mut LegacyRandom);

impl mod_api::RandomSource for ProviderRandom<'_> {
    fn next_int(&mut self) -> i32 {
        self.0.next_int()
    }
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.0.next_int_bound(bound).unwrap_or(0)
    }
    fn next_long(&mut self) -> i64 {
        self.0.next_long()
    }
    fn next_boolean(&mut self) -> bool {
        self.0.next_boolean()
    }
    fn next_float(&mut self) -> f32 {
        self.0.next_float()
    }
    fn next_double(&mut self) -> f64 {
        self.0.next_double()
    }
    fn next_gaussian(&mut self) -> f64 {
        self.0.next_gaussian()
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
        options: &ParticleOptions,
        p: [f64; 3],
        v: [f64; 3],
        seed: i64,
        scope: SpawnScope<'_, '_>,
    ) -> Option<Particle> {
        let Some(entry) = self.mod_sets.get(name) else {
            if let Some(factory) = self.provider_factories.get(name) {
                log::warn!(
                    "Particle provider {name} from mod {}: spawn has no sprite set",
                    factory.mod_id()
                );
            }
            return None;
        };
        let (set, behaviour, group) = (entry.set, entry.behaviour, entry.group);
        let provider = match &entry.provider {
            Binding::Declarative => None,
            Binding::Unavailable { mod_id } => {
                log::warn!(
                    "Particle provider {name} from mod {mod_id}: spawn refused because the provider did not bind"
                );
                return None;
            }
            Binding::Bound(provider) => Some(provider),
        };
        let mut r = LegacyRandom::new(seed);
        let mut q = Particle::moving(&mut r, ParticleType::Smoke, p, v);
        q.lifetime = 20 + r.next_int_bound(20).unwrap_or(0);
        q.set = Some(set);
        q.group = group;
        let sprites = &self.set_sprites[set as usize];
        let mut frame = r.next_int_bound(sprites.len() as i32).unwrap_or(0) as usize;
        if let Some(behaviour) = behaviour {
            behaviour.apply(&mut q, v, &mut r);
            if behaviour.starts_on_first_sprite() {
                frame = 0;
            }
        }
        q.sprite = Sprite::Index(sprites[frame]);
        if let Some(provider) = provider {
            let callbacks = &provider.callbacks;
            let owner = &callbacks.create;
            let Some((level, states)) = scope
                .level
                .and_then(|level| (self.block_state_source)().map(|states| (level, states)))
            else {
                log::warn!(
                    "Particle provider {name} from mod {}: spawn has no live level or block states",
                    owner.id()
                );
                return None;
            };
            let converted;
            let options = if let Some(original) = scope.mod_options {
                original
            } else if let Some(value) = self.options_to_api.and_then(|convert| convert(options)) {
                converted = value;
                &converted
            } else {
                log::warn!(
                    "Particle provider {name} from mod {}: spawn options could not be converted",
                    owner.id()
                );
                return None;
            };
            let level = ProviderLevel {
                level,
                states: (*states).as_ref(),
            };
            let mut random = ProviderRandom(&mut self.random);
            let mut spawn = particle_spawn(options, p, v, &level, &mut random);
            let mut provided = Provided {
                callbacks: Arc::clone(callbacks),
                instance: None,
                presentation: Presentation {
                    frame,
                    facing: ParticleFacing::Camera,
                },
            };
            let mut view = LiveParticle {
                particle: &mut q,
                presentation: &mut provided.presentation,
                context: TickContext {
                    level: level.level,
                    sprites,
                    callbacks,
                },
            };
            if let Err(error) = owner.call("particle provider", "create_custom_particle", || {
                provided.instance = provider
                    .provider
                    .as_ref()
                    .expect("bound provider")
                    .create_particle(&mut spawn, &mut view);
            }) {
                log::warn!("Particle provider {name}: {error}");
                return None;
            }
            // None is the provider's deliberate decision not to spawn, not an error.
            provided.instance.as_ref()?;
            q.set = Some(set);
            q.behaviour = Behaviour::Controlled { translucent: true };
            q.provided = Some(Box::new(provided));
        }
        Some(q)
    }
}

#[cfg(test)]
pub(super) mod tests;
