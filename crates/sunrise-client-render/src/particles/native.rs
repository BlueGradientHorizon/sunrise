//! Opt-in, compiled particle behavior. The ordinary engine still owns the
//! particle, its sprite set, interpolation, rendering and total count limits.
use super::{Particle, ParticleLevel, ParticleOptions, polytone_gpu};
use mod_api::{
    BlockState, LightLayer, ModParticle, ParticleFacing, ParticleLevelView, ParticleProvider,
};
use std::{collections::BTreeMap, fmt::Debug, sync::Arc, sync::OnceLock};

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
    fn clone_box(&self) -> Box<dyn Controller>;
}
impl<T: ParticleController> Controller for T {
    fn tick(&mut self, particle: &mut Particle, level: &ParticleLevel<'_>) -> ParticleUpdate {
        ParticleController::tick(self, particle, level)
    }
    fn clone_box(&self) -> Box<dyn Controller> {
        Box::new(self.clone())
    }
}
impl Clone for Box<dyn Controller> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// The mods' particle providers by type name: set once at start, before the resources first load,
/// and read by every engine built after.
static REGISTERED: OnceLock<BTreeMap<String, Arc<dyn ParticleProvider>>> = OnceLock::new();

/// Sets the native and wasm mods' particle providers (`register_particle_providers`), once, before
/// the resources first load; each engine then resolves its sprite sets' providers at every load.
/// False when already set.
pub fn register_particle_providers(providers: BTreeMap<String, Arc<dyn ParticleProvider>>) -> bool {
    REGISTERED.set(providers).is_ok()
}

/// What [`register_particle_providers`] set; none before.
pub(super) fn registered() -> &'static BTreeMap<String, Arc<dyn ParticleProvider>> {
    static NONE: BTreeMap<String, Arc<dyn ParticleProvider>> = BTreeMap::new();
    REGISTERED.get().unwrap_or(&NONE)
}

/// How far from the origin a provider may move a particle: the world border's limit. Past it, or
/// not a number, the particle expires rather than reach the renderer.
const LIMIT: f64 = 3.0e7;

/// A particle of a mod's code-defined type. It owns the particle's behaviour for the engine: age
/// and expiry stay the engine's, the motion is the provider's, called once per type per tick with
/// every live particle of the type ([`Batch::tick`]). Holds the provider the type resolved to when
/// its sprite sets loaded, so a tick dispatches straight to it.
#[derive(Clone)]
pub(super) struct Provided {
    provider: Arc<dyn ParticleProvider>,
    /// The provider's dense slot among the registered ones, which its batch is filled by.
    slot: u32,
    /// `ModParticle::state`, kept from one tick to the next.
    state: [f64; 4],
    /// The frame of the sprite set drawn, as the provider last saw it.
    frame: u32,
    /// The engine aged the particle this tick and the provider has not run on it yet.
    pending: bool,
    /// A world-fixed orientation (`ParticleFacing::Fixed`), turning the quad before `roll`.
    pub(super) fixed_rotation: Option<glam::Quat>,
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
    pub(super) fn new(provider: Arc<dyn ParticleProvider>, slot: u32, frame: u32) -> Self {
        Self {
            provider,
            slot,
            state: [0.0; 4],
            frame,
            pending: false,
            fixed_rotation: None,
        }
    }
    /// The engine's part of the tick: age, `previous` and expiry. A particle that survives it
    /// joins this tick's batch.
    pub(super) fn age(&mut self, particle: &mut Particle) {
        self.pending = particle.advance_age();
    }
}

/// The batches of one tick, kept between ticks so a tick allocates nothing once warm. One pass
/// over the particles sorts them into a bucket per provider (by the dense slot the provider was
/// given when it registered); each provider then runs once on its bucket.
#[derive(Default)]
pub(super) struct Batch {
    /// The particle indices of each provider slot's batch.
    buckets: Vec<Vec<usize>>,
    views: Vec<ModParticle>,
}
impl Batch {
    /// Calls each provider once with every particle of `particles` it owns that the engine aged
    /// this tick, then takes the results back: a particle that leaves a number past the world's
    /// limit or not finite expires.
    pub(super) fn tick(
        &mut self,
        particles: &mut [Particle],
        level: &dyn ParticleLevelView,
        mut set_frame: impl FnMut(&mut Particle, usize),
    ) {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        for (i, p) in particles.iter().enumerate() {
            if let Some(provided) = p.provided.as_ref().filter(|q| q.pending) {
                let slot = provided.slot as usize;
                if self.buckets.len() <= slot {
                    self.buckets.resize_with(slot + 1, Vec::new);
                }
                self.buckets[slot].push(i);
            }
        }
        for bucket in &self.buckets {
            let Some(&first) = bucket.first() else {
                continue;
            };
            let provider = particles[first]
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
            provider.tick(&mut batch, level);
            self.views = mod_api::__private::particle_batch_into_vec(batch);
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
        match (
            self.provided.as_ref().and_then(|q| q.fixed_rotation),
            self.orientation,
        ) {
            (Some(q), _) => ParticleFacing::Fixed(q.to_array()),
            (None, polytone_gpu::Orientation::AroundY) => ParticleFacing::AroundY,
            _ => ParticleFacing::Camera,
        }
    }
    /// What a provider's tick left of the particle, validated as the renderer needs it.
    fn take(&mut self, view: &ModParticle) -> Option<usize> {
        let finite = |v: &[f64; 3]| v.iter().all(|c| c.abs() <= LIMIT);
        let fixed = match view.facing {
            ParticleFacing::Fixed(q) => {
                let q = glam::Quat::from_array(q);
                let length_squared = q.length_squared();
                (length_squared.is_finite() && length_squared > 0.0).then(|| Some(q.normalize()))
            }
            _ => Some(None),
        };
        let provided = self.provided.as_mut()?;
        provided.pending = false;
        let Some(fixed) = fixed else {
            self.remove();
            return None;
        };
        if view.removed
            || !finite(&view.position)
            || !finite(&view.previous)
            || !finite(&view.velocity)
            || !view.size.is_finite()
            || !view.roll.is_finite()
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
        self.orientation = match view.facing {
            ParticleFacing::AroundY => polytone_gpu::Orientation::AroundY,
            _ => polytone_gpu::Orientation::Camera,
        };
        provided.fixed_rotation = fixed;
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
        // An unloaded chunk reads as full sky and no block light.
        let light = self
            .level
            .chunk(x >> 4, z >> 4)
            .map_or(0xF0, |c| c.light(x & 15, y, z & 15));
        match layer {
            LightLayer::Block => light & 15,
            LightLayer::Sky => light >> 4,
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::*;
    use crate::chunk::{Chunk, Light};
    use mod_api::{
        BlockState, LightLayer, ModParticle, ParticleBatch, ParticleFacing, ParticleLevelView,
        ParticleProvider,
    };
    use std::sync::Arc;
    use {
        sunrise_core::nbt::Tag,
        sunrise_world::{
            DATA_VERSION,
            collision::CachedCollisionRegistry,
            registry::{BlockRegistry, BlockStateId},
        },
    };
    #[derive(Clone, Debug)]
    struct Moving(u32);
    impl ParticleController for Moving {
        fn tick(&mut self, p: &mut Particle, _: &ParticleLevel<'_>) -> ParticleUpdate {
            if p.advance_age() {
                self.0 += 1;
                p.position[0] += f64::from(self.0);
            }
            ParticleUpdate {
                frame: Some(self.0 as usize),
                ..Default::default()
            }
        }
    }
    /// An empty world with no blocks, which a tick reads nothing of.
    pub(in crate::particles) struct Scene {
        world: World,
        facts: BlockFacts,
        collision: BlockCollisionRegistry<'static>,
    }
    impl Scene {
        pub(in crate::particles) fn new() -> Self {
            // Leaked: the collision registry borrows it for as long as the scene lives.
            let registry: &'static BlockRegistry = Box::leak(Box::new(
                BlockRegistry::from_nbt(&Tag::compound([
                    ("DataVersion", Tag::Int(DATA_VERSION)),
                    (
                        "states",
                        Tag::List(vec![Tag::String("minecraft:air".into())]),
                    ),
                    ("defaults", Tag::compound([("minecraft:air", Tag::Int(0))])),
                ]))
                .unwrap(),
            ));
            let cached = CachedCollisionRegistry::from_nbt(
                &Tag::compound([
                    ("DataVersion", Tag::Int(DATA_VERSION)),
                    (
                        "states",
                        Tag::List(vec![Tag::String("minecraft:air".into())]),
                    ),
                    ("shapes", Tag::List(vec![])),
                    ("cached_shapes", Tag::IntArray(vec![-1])),
                ]),
                registry,
            )
            .unwrap();
            let collision = BlockCollisionRegistry::new(std::sync::Arc::new(cached)).unwrap();
            let facts = BlockFacts {
                states: vec![],
                air: BlockStateId(0),
                woods: HashMap::new(),
                sign_texts: HashMap::new(),
            };
            Self {
                world: World::new(BlockStateId(0), 0),
                facts,
                collision,
            }
        }
        pub(in crate::particles) fn level(&self) -> ParticleLevel<'_> {
            ParticleLevel {
                world: &self.world,
                facts: &self.facts,
                collision: &self.collision,
                player: None,
                cached_chunk: std::cell::Cell::new(None),
            }
        }
    }
    fn engine() -> ParticleEngine {
        let mut e = ParticleEngine::new(
            &ResourceSource::indexed(Default::default()),
            &HashMap::new(),
            &HashSet::new(),
        );
        // The game's table of block states; the providers here read none of them.
        e.block_states = Some(Arc::new(Vec::<BlockState>::new()));
        e.mod_sets.insert(
            "example:orbit".into(),
            ModSet {
                set: 0,
                behaviour: None,
                provider: None,
                group: None,
            },
        );
        e.set_sprites.push(vec![0, 1]);
        e.rects = vec![
            Some(Rect {
                u0: 0.0,
                v0: 0.0,
                u1: 1.0,
                v1: 1.0,
                w: 16,
                h: 16
            });
            2
        ];
        e
    }
    #[test]
    fn controlled_particle_keeps_owned_motion_sprite_interpolation_and_expiry() {
        let scene = Scene::new();
        let level = scene.level();
        let mut e = engine();
        let mut p = e
            .controlled("example:orbit", [0.0; 3], true, Moving(0))
            .unwrap();
        p.lifetime = 2;
        p.rgba[3] = 0.5;
        p.velocity = [99.0; 3]; // no vanilla drift may leak into the custom tick
        let mut copy = p.clone();
        e.tick_particle(&mut p, &level);
        e.tick_particle(&mut p, &level);
        e.tick_particle(&mut copy, &level);
        assert_eq!((p.position, p.previous), ([3.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
        assert_eq!(copy.position, [1.0, 0.0, 0.0]); // controller state cloned independently
        assert!(matches!(p.sprite, Sprite::Index(1))); // clamped frame
        p.size = 0.1;
        e.particles.push(p.clone());
        let mut vertices = Vec::new();
        e.render(
            DVec3::ZERO,
            glam::Quat::IDENTITY,
            0.5,
            &|_| 0,
            &mut vertices,
        );
        assert_eq!(vertices.len(), 4);
        assert!(vertices.iter().all(
            |v| v.color >> 24 == 127 && v.layer & crate::gfx::TRANSLUCENT_PARTICLE_OVERLAY != 0
        ));
        e.particles[0].set_visible(false);
        vertices.clear();
        e.render(
            DVec3::ZERO,
            glam::Quat::IDENTITY,
            0.5,
            &|_| 0,
            &mut vertices,
        );
        assert!(vertices.is_empty());
        assert_eq!(e.particles[0].age, 2);
        e.push(copy);
        e.reload(
            &ResourceSource::indexed(Default::default()),
            &HashMap::new(),
            &HashSet::new(),
        );
        assert!(e.pending.is_empty());
        vertices.clear();
        e.render(
            DVec3::ZERO,
            glam::Quat::IDENTITY,
            0.5,
            &|_| 0,
            &mut vertices,
        );
        assert!(vertices.is_empty());
        e.tick_particle(&mut p, &level);
        assert!(p.removed);
        assert_eq!(p.position, [3.0, 0.0, 0.0]);
    }
    /// Counts its ticks, so a test sees which calls a particle's expiry skipped.
    #[derive(Default)]
    struct Probe(std::sync::atomic::AtomicUsize);
    impl ParticleProvider for Probe {
        fn tick(&self, particles: &mut ParticleBatch, _: &dyn ParticleLevelView) {
            for particle in particles {
                self.one(particle);
            }
        }
    }
    impl Probe {
        fn one(&self, particle: &mut ModParticle) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            particle.position[0] = f64::from(particle.age) * 2.0;
            particle.position[1] = particle.state[0];
            particle.state[0] += 0.5;
            particle.frame = particle.age as u32;
            if particle.age == 3 {
                particle.removed = true;
                particle.position[0] = 999.0;
            }
        }
    }
    struct Broken(fn(&mut ModParticle));
    impl ParticleProvider for Broken {
        fn tick(&self, particles: &mut ParticleBatch, _: &dyn ParticleLevelView) {
            for particle in particles {
                self.0(particle);
            }
        }
    }
    /// One engine tick of a single provider particle: the age step, then the provider's batch.
    fn tick_one(e: &mut ParticleEngine, p: &mut Particle, level: &ParticleLevel<'_>) {
        e.tick_particle(p, level);
        e.tick_providers(std::slice::from_mut(p), level);
    }
    /// Counts its calls and the particles each held.
    #[derive(Default)]
    struct Calls(std::sync::Mutex<Vec<usize>>);
    impl ParticleProvider for Calls {
        fn tick(&self, particles: &mut ParticleBatch, _: &dyn ParticleLevelView) {
            self.0.lock().unwrap().push(particles.len());
        }
    }
    #[test]
    fn a_provider_runs_once_per_tick_on_all_its_particles() {
        let scene = Scene::new();
        let level = scene.level();
        let mut e = engine();
        let calls = Arc::new(Calls::default());
        let mut all: Vec<Particle> = (0..3).map(|_| provided(&mut e, calls.clone())).collect();
        for p in &mut all {
            p.lifetime = 20;
            e.tick_particle(p, &level);
        }
        e.tick_providers(&mut all, &level);
        assert_eq!(*calls.0.lock().unwrap(), [3]);
        // Nothing aged since: no particle is pending, so no call.
        e.tick_providers(&mut all, &level);
        assert_eq!(calls.0.lock().unwrap().len(), 1);
    }
    fn provided(e: &mut ParticleEngine, provider: Arc<dyn ParticleProvider>) -> Particle {
        e.providers.insert("example:orbit".into(), provider.clone());
        e.mod_sets.get_mut("example:orbit").unwrap().provider = Some((0, provider));
        e.make(
            &ParticleOptions::named("example:orbit"),
            [1.0; 3],
            [0.0; 3],
            5,
        )
        .unwrap()
    }
    #[test]
    fn a_providers_particle_moves_changes_frame_and_expires_by_its_tick() {
        let scene = Scene::new();
        let level = scene.level();
        let mut e = engine();
        let probe = Arc::new(Probe::default());
        let mut p = provided(&mut e, probe.clone());
        p.lifetime = 20;
        let mut copy = p.clone();
        tick_one(&mut e, &mut p, &level);
        assert_eq!((p.position, p.previous), ([2.0, 0.0, 1.0], [1.0; 3]));
        tick_one(&mut e, &mut p, &level);
        assert_eq!((p.position, p.previous), ([4.0, 0.5, 1.0], [2.0, 0.0, 1.0]));
        assert!(matches!(p.sprite, Sprite::Index(1))); // frame 2, clamped to the 2-sprite set
        // State is per particle: the copy starts from zero state, not the original's.
        tick_one(&mut e, &mut copy, &level);
        assert_eq!(copy.position, [2.0, 0.0, 1.0]);
        // The third tick of `p` removes it before its 20-tick lifetime, and its writes are dropped.
        tick_one(&mut e, &mut p, &level);
        assert!(p.removed);
        assert_eq!(p.position, [4.0, 0.5, 1.0]);
    }
    #[test]
    fn the_engine_expires_a_providers_particle_at_its_lifetime_without_calling_the_provider() {
        let scene = Scene::new();
        let level = scene.level();
        let mut e = engine();
        let probe = Arc::new(Probe::default());
        let mut p = provided(&mut e, probe.clone());
        p.lifetime = 2;
        for _ in 0..2 {
            tick_one(&mut e, &mut p, &level);
            assert!(!p.removed);
        }
        tick_one(&mut e, &mut p, &level);
        assert!(p.removed);
        assert_eq!(probe.0.load(std::sync::atomic::Ordering::Relaxed), 2);
    }
    #[test]
    fn a_provider_cannot_hand_the_renderer_a_bad_number() {
        let scene = Scene::new();
        let level = scene.level();
        let mut e = engine();
        let bad: [fn(&mut ModParticle); 5] = [
            |p| p.position[1] = f64::NAN,
            |p| p.position[0] = f64::INFINITY,
            |p| p.position[2] = 1.0e300,
            |p| p.velocity[0] = f64::NEG_INFINITY,
            |p| p.size = f32::NAN,
        ];
        for tick in bad {
            let mut p = provided(&mut e, Arc::new(Broken(tick)));
            tick_one(&mut e, &mut p, &level);
            assert!(p.removed);
            assert_eq!(p.position, [1.0; 3]);
        }
        let mut p = provided(
            &mut e,
            Arc::new(Broken(|p| {
                p.size = 1.0e9;
                p.rgba = [2.0, -1.0, 0.5, 9.0];
                p.frame = u32::MAX;
                p.lifetime = i32::MIN;
                p.age = i32::MAX;
            })),
        );
        tick_one(&mut e, &mut p, &level);
        assert!(!p.removed);
        assert_eq!((p.size, p.rgba), (16.0, [1.0, 0.0, 0.5, 1.0]));
        assert!(matches!(p.sprite, Sprite::Index(1)));
        assert_eq!((p.lifetime, p.age), (i32::MIN, 1)); // the age stays the engine's
        tick_one(&mut e, &mut p, &level);
        assert!(p.removed);
    }
    /// Hands back a batch of another size, as only a guest's raw bytes could.
    struct Replacing;
    impl ParticleProvider for Replacing {
        fn tick(&self, particles: &mut ParticleBatch, _: &dyn ParticleLevelView) {
            *particles = mod_api::__private::particle_batch_from_vec(Vec::new());
        }
    }
    #[test]
    fn a_batch_handed_back_at_another_size_expires_its_particles() {
        let scene = Scene::new();
        let level = scene.level();
        let mut e = engine();
        let mut p = provided(&mut e, Arc::new(Replacing));
        p.lifetime = 20;
        let at = p.position;
        tick_one(&mut e, &mut p, &level);
        assert!(p.removed);
        assert_eq!(p.position, at);
    }
    /// Reads the batch's two nonfinite-length quaternions.
    #[test]
    fn a_quaternion_with_an_infinite_squared_length_expires_the_particle() {
        let scene = Scene::new();
        let level = scene.level();
        let mut e = engine();
        let facings = [
            ParticleFacing::Fixed([1.0e30, 0.0, 0.0, 0.0]),
            ParticleFacing::Fixed([f32::NAN, 0.0, 0.0, 1.0]),
            ParticleFacing::Fixed([0.0; 4]),
        ];
        for facing in facings {
            struct Set(ParticleFacing);
            impl ParticleProvider for Set {
                fn tick(&self, particles: &mut ParticleBatch, _: &dyn ParticleLevelView) {
                    for particle in particles {
                        particle.facing = self.0;
                    }
                }
            }
            let mut p = provided(&mut e, Arc::new(Set(facing)));
            tick_one(&mut e, &mut p, &level);
            assert!(p.removed, "{facing:?}");
        }
    }
    /// A particle drawn at `center` by a provider's facing and roll, as the renderer builds it.
    fn drawn(
        facing: ParticleFacing,
        roll: f32,
        center: [f64; 3],
        base: glam::Quat,
    ) -> Vec<[f32; 3]> {
        struct Pose(ParticleFacing, f32, [f64; 3]);
        impl ParticleProvider for Pose {
            fn tick(&self, particles: &mut ParticleBatch, _: &dyn ParticleLevelView) {
                for particle in particles {
                    particle.position = self.2;
                    particle.previous = self.2;
                    particle.size = 1.0;
                    particle.facing = self.0;
                    particle.roll = self.1;
                }
            }
        }
        let scene = Scene::new();
        let level = scene.level();
        let mut e = engine();
        let mut p = provided(&mut e, Arc::new(Pose(facing, roll, center)));
        p.lifetime = 20;
        tick_one(&mut e, &mut p, &level);
        e.particles.push(p);
        let mut vertices = Vec::new();
        e.render(DVec3::ZERO, base, 0.5, &|_| 0, &mut vertices);
        assert_eq!(vertices.len(), 4);
        vertices.iter().map(|v| v.pos).collect()
    }
    #[test]
    fn a_providers_fixed_facing_and_roll_turn_the_drawn_quad() {
        // Lying flat facing up, then rolled: the roll interpolates from 0 (the previous tick's)
        // to 1 over the tick, so half a tick in it is 0.5 radians.
        let flat = std::f32::consts::FRAC_1_SQRT_2;
        let corners = drawn(
            ParticleFacing::Fixed([-flat, 0.0, 0.0, flat]),
            1.0,
            [0.0; 3],
            glam::Quat::IDENTITY,
        );
        // The quad's corners in the vertex order, turned by the normalised quaternion and then
        // by `Quaternionf.rotationZ(0.5)` in f32, the order and types vanilla's JOML uses.
        let roll = crate::camera::joml::Quat::rotation_z(0.5);
        let rotation = glam::Quat::from_xyzw(-flat, 0.0, 0.0, flat).normalize()
            * glam::Quat::from_xyzw(roll.x, roll.y, roll.z, roll.w);
        let expected: Vec<[f32; 3]> = [(1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)]
            .map(|(x, y)| (rotation * glam::Vec3::new(x, y, 0.0)).to_array())
            .to_vec();
        assert_eq!(corners, expected);
        // Lying flat: every corner is level with the centre.
        assert!(corners.iter().all(|c| c[1] == 0.0), "{corners:?}");
    }
    #[test]
    fn a_providers_around_y_facing_ignores_the_cameras_pitch() {
        let pitched = glam::Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let around = drawn(ParticleFacing::AroundY, 0.0, [0.0, 0.0, -4.0], pitched);
        assert!(around.iter().all(|c| c[2] == -4.0), "{around:?}");
        let camera = drawn(ParticleFacing::Camera, 0.0, [0.0, 0.0, -4.0], pitched);
        assert!(
            camera.iter().any(|c| (c[2] + 4.0).abs() > 0.5),
            "{camera:?}"
        );
    }
    #[test]
    fn a_second_tick_reuses_the_batch_storage() {
        let scene = Scene::new();
        let level = scene.level();
        let mut e = engine();
        let probe = Arc::new(Probe::default());
        let mut all: Vec<Particle> = (0..3).map(|_| provided(&mut e, probe.clone())).collect();
        let tick = |e: &mut ParticleEngine, all: &mut Vec<Particle>| {
            for p in all.iter_mut() {
                p.lifetime = 20;
                e.tick_particle(p, &level);
            }
            e.tick_providers(all, &level);
        };
        tick(&mut e, &mut all);
        let shape = |e: &ParticleEngine| {
            (
                e.batch.views.as_ptr(),
                e.batch.views.capacity(),
                e.batch.buckets.len(),
                e.batch.buckets[0].as_ptr(),
                e.batch.buckets[0].capacity(),
            )
        };
        let warm = shape(&e);
        tick(&mut e, &mut all);
        assert_eq!(shape(&e), warm);
        assert_eq!(probe.0.load(std::sync::atomic::Ordering::Relaxed), 6);
    }
    #[test]
    fn a_provider_does_not_run_while_the_game_has_no_block_state_table() {
        let scene = Scene::new();
        let level = scene.level();
        let mut e = engine();
        let probe = Arc::new(Probe::default());
        let mut p = provided(&mut e, probe.clone());
        p.lifetime = 20;
        e.set_block_states(None);
        tick_one(&mut e, &mut p, &level);
        assert_eq!(probe.0.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert_eq!(p.position, [1.0; 3]);
        assert!(!p.removed);
    }
    /// The provider's view reads a chunk's states by engine state id and its stored light: a
    /// stone block and a lit section in a loaded chunk, and the chunks and heights without one.
    #[test]
    #[ignore = "daily"]
    fn the_providers_level_view_reads_the_loaded_chunks_states_and_light() {
        let root = crate::mesher::mod_tint_tests::resources().expect("prepared resources");
        let assets = crate::assets::Assets::load(&root).unwrap();
        let values = crate::mesher::mod_tint_tests::tint_values(assets.registry);
        let stone = assets.registry.default_state("minecraft:stone").unwrap();
        let mut scene = Scene::new();
        let mut chunk = Chunk::empty(0, 0);
        chunk.set(1, 64, 3, stone.0);
        let section = chunk.section_index(64).unwrap();
        // Sky 11 over block 4 everywhere in the section.
        Arc::make_mut(&mut chunk.sections[section]).light = Light::packed(Box::new([0xB4; 4096]));
        scene.world.chunks.insert((0, 0), Arc::new(chunk));
        let level = scene.level();
        let view = super::ProviderLevel {
            level: &level,
            states: &values.states,
        };
        assert_eq!(
            view.block_state([1, 64, 3]),
            values.states[stone.0 as usize]
        );
        assert_eq!(view.block_state([2, 64, 3]), values.states[0]);
        assert_ne!(view.block_state([1, 64, 3]), view.block_state([2, 64, 3]));
        assert_eq!(view.brightness(LightLayer::Sky, [1, 64, 3]), 11);
        assert_eq!(view.brightness(LightLayer::Block, [1, 64, 3]), 4);
        // Another chunk is not loaded: the stored light of none, full sky.
        assert_eq!(view.brightness(LightLayer::Sky, [100, 64, 3]), 15);
        assert_eq!(view.brightness(LightLayer::Block, [100, 64, 3]), 0);
        // Above and below the loaded chunk's column.
        assert_eq!(view.brightness(LightLayer::Sky, [1, 10_000, 3]), 15);
        assert_eq!(view.brightness(LightLayer::Sky, [1, -10_000, 3]), 0);
    }
    #[test]
    fn a_type_resolves_to_its_provider_at_load_and_other_types_stay_declarative() {
        let dir =
            std::env::temp_dir().join(format!("sunrise-provider-sets-{}", std::process::id()));
        let particles = dir.join("assets/mymod/particles");
        std::fs::create_dir_all(&particles).unwrap();
        for name in ["orbit", "plain"] {
            std::fs::write(
                particles.join(format!("{name}.json")),
                br#"{"textures":["mymod:one","mymod:two"],"sunrise:behaviour":{"lifetime":7,"size":[0.3,0.3]}}"#,
            )
            .unwrap();
        }
        let mut e = engine();
        e.providers
            .insert("mymod:orbit".into(), Arc::new(Probe::default()));
        e.providers
            .insert("mymod:unmatched".into(), Arc::new(Probe::default()));
        e.reload(
            &ResourceSource::directory(dir.clone()),
            &HashMap::new(),
            &HashSet::new(),
        );
        assert!(e.mod_sets["mymod:orbit"].provider.is_some());
        assert!(e.mod_sets["mymod:plain"].provider.is_none());
        assert!(!e.mod_sets.contains_key("mymod:unmatched"));
        let orbit = e
            .make(
                &ParticleOptions::named("mymod:orbit"),
                [0.0; 3],
                [0.0; 3],
                3,
            )
            .unwrap();
        // The declared spawn values seed it, and its sprite is drawn by frame from the set.
        assert!(orbit.provided.is_some());
        assert_eq!((orbit.lifetime, orbit.size), (7, 0.3));
        assert_eq!(orbit.set, Some(e.mod_sets["mymod:orbit"].set));
        let plain = e
            .make(
                &ParticleOptions::named("mymod:plain"),
                [0.0; 3],
                [0.0; 3],
                3,
            )
            .unwrap();
        assert!(plain.provided.is_none());
        assert!(matches!(plain.behaviour, Behaviour::Mod { .. }));
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn controlled_spawn_respects_distance_settings_and_declared_sprite_sets() {
        let mut e = engine();
        assert!(
            e.controlled("example:missing", [0.0; 3], true, Moving(0))
                .is_none()
        );
        assert!(
            e.controlled("example:orbit", [33.0, 0.0, 0.0], true, Moving(0))
                .is_none()
        );
        assert!(
            e.controlled("example:orbit", [f64::NAN, 0.0, 0.0], true, Moving(0))
                .is_none()
        );
        assert!(
            e.controlled("example:orbit", [32.0, 0.0, 0.0], true, Moving(0))
                .is_some()
        );
        e.particle_level = 2;
        assert!(
            e.controlled("example:orbit", [0.0; 3], true, Moving(0))
                .is_none()
        );
    }
}
