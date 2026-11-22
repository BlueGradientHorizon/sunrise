use super::super::*;
use crate::chunk::{Chunk, Light};
use mod_api::{
    __private::game::{RegistrationContextNew, capture_callback_panic_notices},
    BlockState, CustomParticle, Identifier, LightLayer, ParticleFacing, ParticleLevelView,
    ParticleProvider, ParticleQuad, ParticleSpawn, ParticleTickView, ParticleView,
    RegistrationContext,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use sunrise_assets::resource_source::Resource;
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
/// Freeze a set of providers from one scoped fixture mod, without touching the live registry.
pub(in crate::particles) fn provider_factories<const N: usize>(
    providers: [(&str, Arc<dyn ParticleProvider>); N],
) -> Arc<native::Factories> {
    struct Shared(Arc<dyn ParticleProvider>);
    impl ParticleProvider for Shared {
        fn create_particle(
            &self,
            spawn: &mut ParticleSpawn<'_>,
            particle: &mut dyn ParticleTickView,
        ) -> Option<Box<dyn CustomParticle>> {
            self.0.create_particle(spawn, particle)
        }
    }
    struct Register<const N: usize>([(Identifier, Arc<dyn ParticleProvider>); N]);
    #[mod_api::__subscriptions]
    impl<const N: usize> mod_api::Hooks for Register<N> {
        fn register_particle_providers(
            &self,
            _: &RegistrationContext,
            event: &mut mod_api::RegisterParticleProvidersEvent<'_>,
        ) {
            for (id, provider) in &self.0 {
                let provider = Arc::clone(provider);
                event.register_sprite_set(id.clone(), move |_| Shared(Arc::clone(&provider)));
            }
        }
    }

    let providers = providers.map(|(id, provider)| (Identifier::parse(id).unwrap(), provider));
    let mods = vec![mod_api::ModEntry::new(
        "provider_fixture",
        Box::new(Register(providers)),
    )];
    mod_platform::testing::with_mods(mods.into(), || {
        let mut staged = Vec::new();
        mod_api::dispatch_register_particle_providers(&RegistrationContext::new(), &mut staged);
        Arc::new(native::freeze(staged))
    })
}

fn engine() -> ParticleEngine {
    fixture_engine(Arc::default())
}

pub(in crate::particles) fn fixture_options() -> mod_api::ParticleOptions {
    // These fixtures test particle behavior after ingress, not key resolution.
    mod_api::ParticleOptions::simple(mod_api::__private::parts_installation::particle_key(0))
}

fn fixture_pack() -> (ResourceSource, HashMap<String, Rect>) {
    let root = ResourceSource::indexed(
        ["orbit", "peer"]
            .into_iter()
            .map(|name| {
                (
                    format!("assets/example/particles/{name}.json"),
                    Resource::bytes(br#"{"textures":["example:one","example:two"]}"#.to_vec()),
                )
            })
            .collect(),
    );
    let rect = Rect {
        u0: 0.0,
        v0: 0.0,
        u1: 1.0,
        v1: 1.0,
        w: 16,
        h: 16,
    };
    let atlas = HashMap::from([
        ("example:particle/one".into(), rect),
        ("example:particle/two".into(), rect),
    ]);
    (root, atlas)
}

fn fixture_engine(factories: Arc<native::Factories>) -> ParticleEngine {
    let (root, atlas) = fixture_pack();
    let mut e = ParticleEngine::with_factories(&root, &atlas, &HashSet::new(), &[], factories);
    // The game's table of block states; most fixture providers read none of them.
    e.set_block_state_source(|| Some(Arc::new(Vec::<BlockState>::new())));
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
    let mut other = e
        .controlled("example:orbit", [0.0; 3], true, Moving(0))
        .unwrap();
    other.lifetime = 2;
    e.tick_particle(&mut p, &level);
    e.tick_particle(&mut p, &level);
    e.tick_particle(&mut other, &level);
    assert_eq!((p.position, p.previous), ([3.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    assert_eq!(other.position, [1.0, 0.0, 0.0]);
    assert!(matches!(p.sprite, Sprite::Index(1))); // clamped frame
    p.size = 0.1;
    e.particles.push(p);
    let mut vertices = Vec::new();
    e.render(
        DVec3::ZERO,
        glam::Quat::IDENTITY,
        0.5,
        &|_| 0,
        &mut vertices,
    );
    assert_eq!(vertices.len(), 4);
    assert!(
        vertices.iter().all(
            |v| v.color >> 24 == 127 && v.layer & crate::gfx::TRANSLUCENT_PARTICLE_OVERLAY != 0
        )
    );
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
    let mut p = e.particles.pop().unwrap();
    e.tick_particle(&mut p, &level);
    assert!(p.removed);
    assert_eq!(p.position, [3.0, 0.0, 0.0]);
    e.particles.push(p);
    e.push(other);
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
}
/// A particle a provider poses on each of its ticks (`rolls[age - 1]`), drawn half a tick
/// into the last one with the camera turned by `base`, as the renderer builds its corners.
fn drawn(
    facing: ParticleFacing,
    rolls: &[f32],
    center: [f64; 3],
    base: glam::Quat,
) -> Vec<[f32; 3]> {
    struct Pose(Arc<[f32]>, ParticleFacing);
    impl ParticleProvider for Pose {
        fn create_particle(
            &self,
            _: &mut ParticleSpawn<'_>,
            particle: &mut dyn ParticleTickView,
        ) -> Option<Box<dyn CustomParticle>> {
            particle.set_lifetime(20);
            particle.set_size(1.0);
            particle.set_facing(self.1);
            Some(Box::new(Roll(Arc::clone(&self.0))))
        }
    }
    struct Roll(Arc<[f32]>);
    impl CustomParticle for Roll {
        fn tick(&mut self, particle: &mut dyn ParticleTickView, _: &dyn ParticleLevelView) {
            if particle.advance_age() {
                particle.set_roll(self.0[particle.age() as usize - 1]);
            }
        }
    }
    let scene = Scene::new();
    let level = scene.level();
    let mut e = fixture_engine(provider_factories([(
        "example:orbit",
        Arc::new(Pose(rolls.into(), facing)),
    )]));
    let mut p = e
        .make(
            &ParticleOptions::named("example:orbit"),
            center,
            [0.0; 3],
            5,
            SpawnScope {
                level: Some(&level),
                mod_options: Some(&fixture_options()),
            },
        )
        .unwrap();
    for _ in rolls {
        e.tick_particle(&mut p, &level);
    }
    assert!(!p.removed);
    e.particles.push(p);
    let mut vertices = Vec::new();
    e.render(DVec3::ZERO, base, 0.5, &|_| 0, &mut vertices);
    assert_eq!(vertices.len(), 4);
    vertices.iter().map(|v| v.pos).collect()
}
// The expected corners below come from running JOML 1.10.9 itself: `Quaternionf.rotateZ`
// of `Mth.lerp(0.5, oRoll, roll)` on the facing's quaternion, then `transform` of the quad's
// corners (1,-1), (1,1), (-1,1), (-1,-1), added to the centre.
#[test]
fn a_providers_fixed_facing_and_roll_turn_the_drawn_quad() {
    // A unit quaternion, rolled from 0 (the first tick's) to 1: half a tick in, 0.5 radians.
    let corners = drawn(
        ParticleFacing::Fixed([-0.5, -0.5, -0.5, 0.5]),
        &[1.0],
        [0.0; 3],
        glam::Quat::IDENTITY,
    );
    assert_eq!(
        corners,
        [
            [-0.39815706, 0.0, 1.3570082],
            [1.3570082, 0.0, 0.39815706],
            [0.39815706, 0.0, -1.3570082],
            [-1.3570082, 0.0, -0.39815706],
        ]
    );
}
#[test]
fn a_providers_camera_facing_rotates_the_quad_by_the_camera_then_the_interpolated_roll() {
    let corners = drawn(
        ParticleFacing::Camera,
        &[4.1, 4.5],
        [3.25, -1.5, 2.75],
        glam::Quat::from_xyzw(0.5, -0.5, 0.5, 0.5),
    );
    assert_eq!(
        corners,
        [
            [3.765367, -1.5, 1.4330349],
            [4.566965, -1.5, 3.265367],
            [2.734633, -1.5, 4.066965],
            [1.9330349, -1.5, 2.234633],
        ]
    );
}
#[test]
fn a_providers_around_y_facing_takes_the_cameras_y_and_w_and_rolls_from_the_last_tick() {
    // The camera's pitch and roll components (x and z) do not reach the quad, and the
    // particle is off the camera's axes, so a yaw taken from its direction would differ.
    let camera = glam::Quat::from_xyzw(0.25, 0.5, -0.25, 0.8);
    let corners = drawn(
        ParticleFacing::AroundY,
        &[0.3, 0.9],
        [3.0, 1.0, -4.0],
        camera,
    );
    assert_eq!(
        corners,
        [
            [3.6090915, 0.7393068, -5.2494183],
            [3.1142364, 2.389978, -4.234331],
            [2.3909085, 1.2606932, -2.7505815],
            [2.8857636, -0.38997805, -3.765669],
        ]
    );
}
#[test]
fn a_roll_that_returns_to_zero_draws_unrolled_as_vanilla_does() {
    // `SingleQuadParticle.extract` rolls only when the current roll is not zero.
    let camera = glam::Quat::from_xyzw(0.25, 0.5, -0.25, 0.8);
    let corners = drawn(
        ParticleFacing::AroundY,
        &[1.0, 0.0],
        [3.0, 1.0, -4.0],
        camera,
    );
    assert_eq!(
        corners,
        [
            [3.4382024, 0.0, -4.898876],
            [3.4382024, 2.0, -4.898876],
            [2.5617976, 2.0, -3.1011236],
            [2.5617976, 0.0, -3.1011236],
        ]
    );
}
#[test]
fn extract_edits_only_this_frame_and_culls_using_its_quad_size() {
    use mod_api::__private::{LoadAccess, ModPhase, PhaseOwner, check_access};

    struct Animated;
    impl ParticleProvider for Animated {
        fn create_particle(
            &self,
            _: &mut ParticleSpawn<'_>,
            particle: &mut dyn ParticleTickView,
        ) -> Option<Box<dyn CustomParticle>> {
            particle.set_previous_position([6.0, 0.0, 0.0]);
            particle.set_size(0.0);
            particle.set_rgba([1.0; 4]);
            Some(Box::new(Self))
        }
    }
    impl CustomParticle for Animated {
        fn extract(&self, particle: &dyn ParticleView, partial: f32, quad: &mut ParticleQuad) {
            check_access("fixture_extract_world_access", LoadAccess::WorldOnly);
            quad.visible = partial != 0.0 && particle.is_alive();
            quad.size += 24.0 * partial;
            quad.facing = if partial == 0.5 {
                // Finite input but no quaternion rotation, so the corners are non-finite.
                ParticleFacing::Fixed([0.0; 4])
            } else {
                // A raw quaternion for a quarter turn about +Z.
                ParticleFacing::Fixed([0.0, 0.0, 1.0, 1.0])
            };
            quad.rgba = [-0.5, 1.25, 0.5, -0.25];
        }
    }

    let mut e = fixture_engine(provider_factories([("example:orbit", Arc::new(Animated))]));
    let scene = Scene::new();
    let level = scene.level();
    e.add_with(
        &ParticleOptions::named("example:orbit"),
        [10.0, 0.0, 0.0],
        [0.0; 3],
        ParticleSpawnFlags::default(),
        SpawnScope::with_level(&level, Some(&fixture_options())),
    );
    e.tick(&level); // admit without ticking the particle
    let camera = DVec3::new(2.0, 0.0, 0.0);
    let mut vertices = Vec::new();
    {
        let _reload = PhaseOwner::scope(ModPhase::Reloading);
        e.render(camera, Quat::IDENTITY, 0.75, &|_| 0, &mut vertices);
        assert!(vertices.is_empty());
        assert!(e.particles[0].is_alive());
    }
    let _world = PhaseOwner::scope(ModPhase::World);
    for partial in [0.0, 0.5] {
        e.render(camera, Quat::IDENTITY, partial, &|_| 0, &mut vertices);
        assert!(vertices.is_empty());
        assert!(e.particles[0].is_alive());
    }
    // At 3/4 of a tick, the center is 9 - 2 and the half-width is 18. A quarter
    // turn maps (x, y) to (-y, x). The original zero-size quad would be subpixel.
    e.render_culled(
        camera,
        Quat::IDENTITY,
        0.75,
        1.0,
        true,
        None,
        |_, _| false,
        &|_| 0,
        &mut vertices,
    );
    assert_eq!(
        vertices.iter().map(|v| v.pos).collect::<Vec<_>>(),
        [
            [25.0, 18.0, 0.0],
            [-11.0, 18.0, 0.0],
            [-11.0, -18.0, 0.0],
            [25.0, -18.0, 0.0],
        ],
    );
    // Floor and mask each channel: -128, 318, 127, -64, in vertex RGBA order.
    assert!(vertices.iter().all(|v| v.color == 0xc07f_3e80));
    let particle = &e.particles[0];
    assert!(particle.is_alive());
    assert_eq!(
        (particle.age, particle.size, particle.rgba),
        (0, 0.0, [1.0; 4])
    );
    e.tick(&level);
    assert_eq!(e.particles[0].age, 1);
}

#[test]
fn extract_panic_disables_only_that_registrations_extraction() {
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    #[derive(Clone)]
    struct Instance {
        draws: Arc<AtomicUsize>,
        panic: bool,
    }
    impl ParticleProvider for Instance {
        fn create_particle(
            &self,
            _: &mut ParticleSpawn<'_>,
            particle: &mut dyn ParticleTickView,
        ) -> Option<Box<dyn CustomParticle>> {
            particle.set_lifetime(20);
            Some(Box::new(self.clone()))
        }
    }
    impl CustomParticle for Instance {
        fn extract(&self, _: &dyn ParticleView, _: f32, quad: &mut ParticleQuad) {
            self.draws.fetch_add(1, Relaxed);
            assert!(!self.panic, "fixture extract panic");
            quad.rgba = [1.0; 4];
        }
    }

    let broken = Instance {
        draws: Arc::default(),
        panic: true,
    };
    let healthy = Instance {
        draws: Arc::default(),
        panic: false,
    };
    let mut e = fixture_engine(provider_factories([
        ("example:orbit", Arc::new(broken.clone())),
        ("example:peer", Arc::new(healthy.clone())),
    ]));
    let scene = Scene::new();
    let level = scene.level();
    let ((), notices) = capture_callback_panic_notices(|| {
        for name in ["example:orbit", "example:orbit", "example:peer"] {
            e.add_with(
                &ParticleOptions::named(name),
                [1.0; 3],
                [0.0; 3],
                ParticleSpawnFlags::default(),
                SpawnScope::with_level(&level, Some(&fixture_options())),
            );
        }
        e.tick(&level);
        for age in [0, 1] {
            let mut vertices = Vec::new();
            e.render(DVec3::ZERO, Quat::IDENTITY, 0.5, &|_| 0, &mut vertices);
            assert_eq!(vertices.len(), 4, "only the healthy particle draws");
            assert_eq!(e.len(), 3, "extraction does not remove particles");
            assert!(e.particles.iter().all(|p| p.age == age && p.is_alive()));
            e.tick(&level);
        }
    });
    assert_eq!(
        (broken.draws.load(Relaxed), healthy.draws.load(Relaxed)),
        (1, 2)
    );
    let [notice] = notices.as_slice() else {
        panic!("expected one toast, got {notices:?}");
    };
    assert_eq!(notice.mod_id, "provider_fixture");
    assert_eq!(notice.operation, "extract_custom_particle");
    assert!(notice.message.contains("fixture extract panic"));
}

fn lit_chunk() -> Chunk {
    let mut chunk = Chunk::empty(0, 0);
    let section = chunk.section_index(64).unwrap();
    Arc::make_mut(&mut chunk.sections[section]).light = Light::packed(Box::new([0xB4; 4096]));
    chunk
}
#[derive(Clone, Default)]
struct SpawnLightProbe(Arc<Mutex<Vec<(f32, u8)>>>);
impl ParticleProvider for SpawnLightProbe {
    fn create_particle(
        &self,
        spawn: &mut ParticleSpawn<'_>,
        _: &mut dyn ParticleTickView,
    ) -> Option<Box<dyn CustomParticle>> {
        self.0.lock().unwrap().push((
            spawn.options.scale,
            spawn.level.brightness(LightLayer::Block, [1, 64, 3]),
        ));
        Some(Box::new(self.clone()))
    }
}
impl CustomParticle for SpawnLightProbe {}

#[test]
fn direct_named_add_lends_the_level_and_applies_the_level_limiters() {
    let provider = SpawnLightProbe::default();
    let mut engine = fixture_engine(provider_factories([(
        "example:orbit",
        Arc::new(provider.clone()),
    )]));
    let mut scene = Scene::new();
    scene.world.chunks.insert((0, 0), Arc::new(lit_chunk()));
    let level = scene.level();
    let mut typed = fixture_options();
    typed.scale = 2.5;
    let scope = SpawnScope::with_level(&level, Some(&typed));
    let options = ParticleOptions::named("example:orbit");
    let near = [1.0, 64.0, 3.0];
    engine.camera = near;
    engine.particle_level = 0;
    engine.add(&options, [34.0, 64.0, 3.0], [0.0; 3], scope);
    engine.particle_level = 2;
    engine.add(&options, near, [0.0; 3], scope);
    assert!(provider.0.lock().unwrap().is_empty());

    engine.particle_level = 0;
    engine.add(&options, near, [0.0; 3], scope);
    assert_eq!(*provider.0.lock().unwrap(), [(2.5, 4)]);
    engine.tick(&level);
    assert_eq!(engine.len(), 1);
}

#[test]
fn named_controller_spawns_lend_the_level_from_both_tick_paths() {
    #[derive(Debug)]
    struct SpawnOrbit;
    impl ParticleController for SpawnOrbit {
        fn tick(&mut self, _: &mut Particle, _: &ParticleLevel<'_>) -> ParticleUpdate {
            ParticleUpdate {
                spawned: vec![(ParticleOptions::named("example:orbit"), [0.0; 3], [0.0; 3])],
                ..Default::default()
            }
        }
    }

    let mut scene = Scene::new();
    scene.world.chunks.insert((0, 0), Arc::new(lit_chunk()));
    let level = scene.level();
    for pre_tick in [true, false] {
        let provider = SpawnLightProbe::default();
        let mut engine = fixture_engine(provider_factories([(
            "example:orbit",
            Arc::new(provider.clone()),
        )]));
        engine.options_to_api = Some(|_| Some(fixture_options()));
        let mut controller = engine
            .controlled("example:orbit", [0.0; 3], false, SpawnOrbit)
            .unwrap();
        controller.pre_tick = pre_tick;
        engine.push(controller);
        engine.tick(&level);
        if !pre_tick {
            assert!(provider.0.lock().unwrap().is_empty());
            engine.tick(&level);
        }
        assert_eq!(*provider.0.lock().unwrap(), [(1.0, 4)]);
    }
}

#[test]
fn direct_create_lends_the_level_without_the_add_particle_limiters() {
    let provider = SpawnLightProbe::default();
    let mut engine = fixture_engine(provider_factories([(
        "example:orbit",
        Arc::new(provider.clone()),
    )]));
    let mut scene = Scene::new();
    scene.world.chunks.insert((0, 0), Arc::new(lit_chunk()));
    let level = scene.level();
    let typed = fixture_options();
    engine.particle_level = 2;
    engine.create(
        &ParticleOptions::named("example:orbit"),
        [100.0, 64.0, 3.0],
        [0.0; 3],
        SpawnScope::with_level(&level, Some(&typed)),
    );
    assert_eq!(*provider.0.lock().unwrap(), [(1.0, 4)]);
    engine.tick(&level);
    assert_eq!(engine.len(), 1);
}

#[test]
fn named_local_sends_lend_the_level_for_single_and_spread_spawns() {
    let provider = SpawnLightProbe::default();
    let mut engine = fixture_engine(provider_factories([(
        "example:orbit",
        Arc::new(provider.clone()),
    )]));
    let mut scene = Scene::new();
    scene.world.chunks.insert((0, 0), Arc::new(lit_chunk()));
    let level = scene.level();
    let typed = fixture_options();
    for count in [0, 2] {
        engine.send_particles(
            &ParticleOptions::named("example:orbit"),
            [0.0; 3],
            [0.0; 3],
            0.0,
            count,
            SpawnScope::with_level(&level, Some(&typed)),
        );
    }
    assert_eq!(*provider.0.lock().unwrap(), [(1.0, 4); 3]);
}

#[test]
fn named_tracking_emitter_lends_the_level_on_its_constructor_tick() {
    let provider = SpawnLightProbe::default();
    let mut engine = fixture_engine(provider_factories([(
        "example:orbit",
        Arc::new(provider.clone()),
    )]));
    let mut scene = Scene::new();
    scene.world.chunks.insert((0, 0), Arc::new(lit_chunk()));
    let body = TrackingBody {
        feet: [0.0; 3],
        width: 1.0,
        height: 2.0,
    };
    engine.seed_randoms(0, 0, 0);
    {
        let level = scene.level();
        let mut typed = fixture_options();
        typed.scale = 2.5;
        engine.tracking_emitter_for(
            ParticleOptions::named("example:orbit"),
            TrackingTarget::Entity(7),
            body,
            2,
            SpawnScope::with_level(&level, Some(&typed)),
        );
    }
    // Vanilla's 16 attempts with Java Random(0) admit seven per burst here.
    assert_eq!(*provider.0.lock().unwrap(), [(2.5, 4); 7]);
    provider.0.lock().unwrap().clear();
    scene.world.chunks.clear();
    engine.tick_tracking(&scene.level(), |_| Some(body));
    assert_eq!(*provider.0.lock().unwrap(), [(2.5, 0); 7]);
}

fn lights(view: &super::ProviderLevel, pos: [i32; 3]) -> (u8, u8) {
    (
        view.brightness(LightLayer::Sky, pos),
        view.brightness(LightLayer::Block, pos),
    )
}
#[test]
fn the_providers_light_reads_follow_the_dimensions_sky() {
    for (shape, sky) in [
        (crate::chunk::LevelShape::OVERWORLD, true),
        (crate::chunk::LevelShape::NETHER, false),
    ] {
        let mut scene = Scene::new();
        scene.world.shape = shape;
        scene.world.chunks.insert((0, 0), Arc::new(lit_chunk()));
        let level = scene.level();
        let view = super::ProviderLevel {
            level: &level,
            states: &[],
        };
        let full = if sky { 15 } else { 0 };
        // The stored nibbles, sky 11 and block 4, but a dimension without skylight has no sky layer.
        assert_eq!(lights(&view, [1, 64, 3]), (if sky { 11 } else { 0 }, 4));
        // No chunk is loaded there: the sky the dimension has, and no block light.
        assert_eq!(lights(&view, [100, 64, 3]), (full, 0));
        // Outside the build height the view uses the render world's light read as well.
        // The world owns padding and retained border light (#2761), not the provider.
        assert_eq!(lights(&view, [1, 10_000, 3]), (full, 0));
    }
}
/// The provider's view reads a chunk by numeric state ID, using the game's air if absent.
#[test]
fn the_providers_level_view_reads_the_loaded_chunks_states() {
    mod_platform::testing::with_mods(Vec::new().into(), || {
        mod_platform::__private::part_tests::install_parts_fixture(&[]).unwrap();
        mod_api::__private::resolve_block_keys(["minecraft:air", "minecraft:stone"], |_| {
            sunrise_mod_runtime::block_definitions::BlockDefinition::new(&[&BTreeMap::new()], 0)
                .map_err(|error| format!("{error:?}"))
        })
        .unwrap();
        let key = |name| mod_api::__private::block_key(name).unwrap();
        let air = key("minecraft:air");
        let stone = key("minecraft:stone");
        let states =
            [air, stone].map(|block| mod_api::__private::block_state_from_ordinal(block, 0));
        let mut scene = Scene::new();
        let mut chunk = Chunk::empty(0, 0);
        chunk.set(1, 64, 3, 1);
        scene.world.chunks.insert((0, 0), Arc::new(chunk));
        let level = scene.level();
        let view = super::ProviderLevel {
            level: &level,
            states: &states,
        };
        assert_eq!(view.block_state([1, 64, 3]).block(), stone);
        assert_eq!(view.block_state([2, 64, 3]).block(), air);
        assert_eq!(view.block_state([100, 64, 3]).block(), air);
    });
}
#[test]
fn owned_particles_construct_before_fifo_admission_tick_in_order_and_drop_once() {
    #[derive(Debug, PartialEq)]
    enum Event {
        Create(i32, i32),
        Tick(i32, i32, i32, f64, f64),
        Drop(i32),
    }
    type Events = Arc<Mutex<Vec<Event>>>;

    struct Start {
        events: Events,
        key: mod_api::ParticleKey,
        stone: BlockState,
        thread: std::thread::ThreadId,
    }
    struct Move {
        events: Events,
        id: i32,
        steps: i32,
        thread: std::thread::ThreadId,
    }
    impl ParticleProvider for Start {
        fn create_particle(
            &self,
            spawn: &mut ParticleSpawn<'_>,
            particle: &mut dyn ParticleTickView,
        ) -> Option<Box<dyn CustomParticle>> {
            assert_eq!(std::thread::current().id(), self.thread);
            assert_eq!((spawn.options.ty, spawn.options.scale), (self.key, 2.5));
            assert_eq!(spawn.velocity, [0.25, 0.0, 0.0]);
            assert_eq!(spawn.position, particle.position());
            assert_eq!(spawn.level.block_state([1, 64, 3]), self.stone);
            assert_eq!(spawn.level.brightness(LightLayer::Block, [1, 64, 3]), 4);
            let id = spawn.position[0] as i32;
            let draw = spawn.random.next_int();
            self.events.lock().unwrap().push(Event::Create(id, draw));
            particle.set_has_physics(false);
            particle.set_velocity([1.0, 0.0, 0.0]);
            particle.set_friction(1.0);
            particle.set_lifetime(if id == 10 { 2 } else { 7 });
            Some(Box::new(Move {
                events: Arc::clone(&self.events),
                id,
                steps: 0,
                thread: self.thread,
            }))
        }
    }
    impl CustomParticle for Move {
        fn tick(&mut self, particle: &mut dyn ParticleTickView, level: &dyn ParticleLevelView) {
            assert_eq!(std::thread::current().id(), self.thread);
            assert_eq!(level.brightness(LightLayer::Sky, [1, 64, 3]), 11);
            self.steps += 1;
            if self.id == 10 {
                particle.tick_base();
            } else if particle.advance_age() {
                particle.move_by([f64::from(self.steps), 0.0, 0.0]);
                if self.steps == 2 {
                    particle.remove();
                }
            }
            self.events.lock().unwrap().push(Event::Tick(
                self.id,
                self.steps,
                particle.age(),
                particle.previous_position()[0],
                particle.position()[0],
            ));
        }
    }
    impl Drop for Move {
        fn drop(&mut self) {
            self.events.lock().unwrap().push(Event::Drop(self.id));
        }
    }

    mod_platform::testing::with_mods(Vec::new().into(), || {
        mod_platform::__private::part_tests::install_parts_fixture(&[]).unwrap();
        mod_api::__private::resolve_particles(["example:orbit"], |_| {
            Some(mod_api::__private::ParticleKind::Custom)
        })
        .unwrap();
        mod_api::__private::resolve_block_keys(["minecraft:air", "minecraft:stone"], |_| {
            sunrise_mod_runtime::block_definitions::BlockDefinition::new(&[&BTreeMap::new()], 0)
                .map_err(|error| format!("{error:?}"))
        })
        .unwrap();
        let key = mod_api::__private::particle_named_key("example:orbit").unwrap();
        let state = |name| {
            mod_api::__private::block_state_from_ordinal(
                mod_api::__private::block_key(name).unwrap(),
                0,
            )
        };
        let events = Events::default();
        let factories = provider_factories([(
            "example:orbit",
            Arc::new(Start {
                events: Arc::clone(&events),
                key,
                stone: state("minecraft:stone"),
                thread: std::thread::current().id(),
            }),
        )]);
        let mut scene = Scene::new();
        let mut chunk = lit_chunk();
        chunk.set(1, 64, 3, 1);
        scene.world.chunks.insert((0, 0), Arc::new(chunk));
        let level = scene.level();
        let mut e = fixture_engine(factories);
        e.set_block_state_source(|| {
            let states = ["minecraft:air", "minecraft:stone"].map(|name| {
                mod_api::__private::block_state_from_ordinal(
                    mod_api::__private::block_key(name).unwrap(),
                    0,
                )
            });
            Some(Arc::new(states))
        });
        e.options_to_api = Some(|options| {
            let key = mod_api::__private::particle_named_key(options.mod_particle.as_deref()?)?;
            let mut converted = mod_api::ParticleOptions::simple(key);
            converted.scale = options.scale;
            Some(converted)
        });
        e.camera = [0.0, 64.0, 0.0];
        e.live_limit = Some(2);
        e.parallel_min = 1;
        e.parallel_threads = 2;
        e.seed_randoms(0, 0, 0);
        let mut options = ParticleOptions::named("example:orbit");
        options.scale = 2.5;
        let mut typed = mod_api::ParticleOptions::simple(key);
        typed.scale = 2.5;
        for x in [40, 10, 20, 30] {
            e.add_with(
                &options,
                [f64::from(x), 64.0, 3.0],
                [0.25, 0.0, 0.0],
                ParticleSpawnFlags::default(),
                SpawnScope::with_level(&level, (x != 20).then_some(&typed)),
            );
        }
        use Event::*;
        // The far-away spawn never reaches the provider. All three near ones are built and
        // draw from java.util.Random(0) before the FIFO admits the first two.
        let created = [
            Create(10, -1155484576),
            Create(20, -723955400),
            Create(30, 1033096058),
        ];
        assert_eq!(*events.lock().unwrap(), created);
        assert_eq!((e.len(), e.pending.len()), (0, 3));
        e.tick(&level);
        assert_eq!((e.len(), e.pending.len()), (2, 0));
        e.tick(&level);
        e.tick(&level);
        assert_eq!(e.len(), 1);
        e.tick(&level);
        assert_eq!(e.len(), 0);
        let mut expected = Vec::from(created);
        expected.extend([
            Drop(30),
            Tick(10, 1, 1, 10.0, 11.0),
            Tick(20, 1, 1, 20.0, 21.0),
            Tick(10, 2, 2, 11.0, 12.0),
            Tick(20, 2, 2, 21.0, 23.0),
            Drop(20),
            Tick(10, 3, 3, 12.0, 12.0),
            Drop(10),
        ]);
        assert_eq!(*events.lock().unwrap(), expected);
    });
}

#[test]
fn a_tick_panic_disables_only_that_registration_across_instances_and_reload() {
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
    #[derive(Default)]
    struct Calls {
        creates: AtomicUsize,
        ticks: AtomicUsize,
        drops: AtomicUsize,
    }
    struct Start(Arc<Calls>, bool);
    struct Tick(Arc<Calls>, bool);
    impl ParticleProvider for Start {
        fn create_particle(
            &self,
            _: &mut ParticleSpawn<'_>,
            _: &mut dyn ParticleTickView,
        ) -> Option<Box<dyn CustomParticle>> {
            self.0.creates.fetch_add(1, Relaxed);
            Some(Box::new(Tick(Arc::clone(&self.0), self.1)))
        }
    }
    impl CustomParticle for Tick {
        fn tick(&mut self, particle: &mut dyn ParticleTickView, _: &dyn ParticleLevelView) {
            self.0.ticks.fetch_add(1, Relaxed);
            assert!(!self.1, "fixture tick panic");
            particle.advance_age();
        }
    }
    impl Drop for Tick {
        fn drop(&mut self) {
            self.0.drops.fetch_add(1, Relaxed);
        }
    }
    let broken = Arc::new(Calls::default());
    let healthy = Arc::new(Calls::default());
    let mut e = fixture_engine(provider_factories([
        ("example:orbit", Arc::new(Start(Arc::clone(&broken), true))),
        ("example:peer", Arc::new(Start(Arc::clone(&healthy), false))),
    ]));
    let scene = Scene::new();
    let level = scene.level();
    let typed = fixture_options();
    let spawn = |e: &mut ParticleEngine, name| {
        e.add_with(
            &ParticleOptions::named(name),
            [1.0; 3],
            [0.0; 3],
            ParticleSpawnFlags::default(),
            SpawnScope::with_level(&level, Some(&typed)),
        );
    };
    let ((), notices) = capture_callback_panic_notices(|| {
        for name in ["example:orbit", "example:orbit", "example:peer"] {
            spawn(&mut e, name);
        }
        e.tick(&level); // admissions only
        assert_eq!(e.len(), 3);
        e.tick(&level);
        assert_eq!(e.len(), 1);
        assert_eq!(broken.ticks.load(Relaxed), 1); // a second instance didn't enter the poisoned tick
        assert_eq!(broken.drops.load(Relaxed), 2);
        assert_eq!(healthy.ticks.load(Relaxed), 1);
        let (root, atlas) = fixture_pack();
        e.reload(&root, &atlas, &HashSet::new());
        assert_eq!(healthy.drops.load(Relaxed), 1);
        for name in ["example:orbit", "example:peer", "example:peer"] {
            spawn(&mut e, name);
        }
        e.tick(&level);
        // Create is a different callback kind and is still enabled even for `orbit`.
        assert_eq!((e.len(), broken.creates.load(Relaxed)), (3, 3));
        e.tick(&level);
        assert_eq!((e.len(), broken.ticks.load(Relaxed)), (2, 1));
        assert_eq!(broken.drops.load(Relaxed), 3);
        assert_eq!(healthy.ticks.load(Relaxed), 3);
        spawn(&mut e, "example:peer");
        e.clear();
        assert_eq!(healthy.drops.load(Relaxed), 4); // live and pending, once each
        assert_eq!(e.len(), 0);
        assert!(e.pending.is_empty());
    });
    let [notice] = notices.as_slice() else {
        panic!("expected one toast, got {notices:?}")
    };
    assert_eq!(notice.mod_id, "provider_fixture");
    assert_eq!(notice.operation, "tick_custom_particle");
    assert!(notice.message.contains("fixture tick panic"));
}

#[test]
fn malformed_writes_are_refused_without_expiry_and_age_sprite_selection_is_exact() {
    struct Probe;
    impl ParticleProvider for Probe {
        fn create_particle(
            &self,
            _: &mut ParticleSpawn<'_>,
            particle: &mut dyn ParticleTickView,
        ) -> Option<Box<dyn CustomParticle>> {
            let position = [31_000_000.0, 0.0, 0.0];
            assert!(particle.set_position(position));
            assert!(!particle.set_position([f64::NAN; 3]));
            assert_eq!(particle.position(), position);
            assert!(particle.set_velocity([f64::MAX, 0.0, 0.0]));
            assert!(!particle.set_velocity([f64::INFINITY; 3]));
            assert!(!particle.move_by([f64::NAN; 3]));
            assert_eq!(particle.velocity(), [f64::MAX, 0.0, 0.0]);
            assert!(particle.set_size(32.0));
            assert!(!particle.set_size(f32::INFINITY));
            assert_eq!(particle.size(), 32.0);
            assert!(particle.set_roll(f32::MAX));
            assert!(!particle.set_roll(f32::NAN));
            assert_eq!(particle.roll(), f32::MAX);
            assert!(particle.set_facing(ParticleFacing::Fixed([0.0, 0.0, 0.0, f32::MAX])));
            assert!(!particle.set_facing(ParticleFacing::Fixed([0.0; 4])));
            assert_eq!(
                particle.facing(),
                ParticleFacing::Fixed([0.0, 0.0, 0.0, 1.0])
            );

            assert!(particle.set_sprite_index(1));
            assert!(!particle.set_sprite_index(2));
            particle.set_lifetime(0);
            assert!(!particle.set_sprite_from_age());
            assert_eq!(particle.sprite_index(), 1);
            particle.set_lifetime(4);
            particle.set_age(3);
            assert!(particle.set_sprite_from_age());
            assert_eq!(particle.sprite_index(), 0);
            particle.set_age(4);
            assert!(particle.set_sprite_from_age());
            assert_eq!(particle.sprite_index(), 1);
            particle.set_age(8);
            assert!(!particle.set_sprite_from_age());
            assert_eq!(particle.sprite_index(), 1);
            assert!(particle.is_alive());

            particle.set_age(i32::MAX);
            particle.set_lifetime(i32::MAX);
            Some(Box::new(Self))
        }
    }
    impl CustomParticle for Probe {}

    let scene = Scene::new();
    let level = scene.level();
    let mut e = fixture_engine(provider_factories([("example:orbit", Arc::new(Probe))]));
    let mut particle = e
        .make(
            &ParticleOptions::named("example:orbit"),
            [0.0; 3],
            [0.0; 3],
            1,
            SpawnScope {
                level: Some(&level),
                mod_options: Some(&fixture_options()),
            },
        )
        .unwrap();
    assert!(particle.is_alive());
    assert_eq!(
        e.snapshot(&particle).quad.unwrap().sprite,
        "example:particle/two"
    );
    e.tick_particle(&mut particle, &level);
    assert!(!particle.is_alive());
    assert_eq!(particle.age, i32::MIN); // Java post-increment, even on the expiry tick.
    assert_eq!(particle.previous, [31_000_000.0, 0.0, 0.0]);
}

#[test]
fn a_phase_refused_tick_does_not_poison_or_retire_the_particle() {
    use mod_api::__private::{LoadAccess, ModPhase, PhaseOwner, check_access};
    struct Probe;
    impl ParticleProvider for Probe {
        fn create_particle(
            &self,
            _: &mut ParticleSpawn<'_>,
            particle: &mut dyn ParticleTickView,
        ) -> Option<Box<dyn CustomParticle>> {
            particle.set_velocity([0.0; 3]);
            Some(Box::new(Self))
        }
    }
    impl CustomParticle for Probe {
        fn tick(&mut self, particle: &mut dyn ParticleTickView, _: &dyn ParticleLevelView) {
            check_access("fixture_provider_world_access", LoadAccess::WorldOnly);
            particle.tick_base();
        }
    }
    let scene = Scene::new();
    let level = scene.level();
    let mut e = fixture_engine(provider_factories([("example:orbit", Arc::new(Probe))]));
    let ((), notices) = capture_callback_panic_notices(|| {
        let mut particle = e
            .make(
                &ParticleOptions::named("example:orbit"),
                [0.0; 3],
                [0.0; 3],
                1,
                SpawnScope {
                    level: Some(&level),
                    mod_options: Some(&fixture_options()),
                },
            )
            .unwrap();
        {
            let _scope = PhaseOwner::scope(ModPhase::Reloading);
            for _ in 0..2 {
                e.tick_particle(&mut particle, &level);
                assert!(particle.is_alive());
                assert_eq!(particle.age, 0);
            }
        }
        let _scope = PhaseOwner::scope(ModPhase::World);
        e.tick_particle(&mut particle, &level);
        assert!(particle.is_alive());
        assert_eq!(particle.age, 1);
    });
    assert!(notices.is_empty());
}

#[test]
fn a_refused_factory_capture_is_disposed_under_its_owner() {
    use mod_api::__private::{LoadAccess, ModPhase, PhaseOwner, check_access};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

    struct PanickingCapture(Arc<AtomicBool>);
    impl Drop for PanickingCapture {
        fn drop(&mut self) {
            self.0.store(true, Relaxed);
            panic!("staged factory capture drop");
        }
    }
    struct Refused(Arc<AtomicBool>);
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Refused {
        fn register_particle_providers(
            &self,
            _: &RegistrationContext,
            event: &mut mod_api::RegisterParticleProvidersEvent<'_>,
        ) {
            let capture = PanickingCapture(Arc::clone(&self.0));
            event.register_sprite_set(Identifier::parse("example:orbit").unwrap(), move |_| {
                std::hint::black_box(&capture);
                SpawnLightProbe::default()
            });
            check_access("fixture_provider_world_access", LoadAccess::WorldOnly);
        }
    }
    struct Later {
        capture_dropped: Arc<AtomicBool>,
        binds: Arc<AtomicUsize>,
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Later {
        fn register_particle_providers(
            &self,
            _: &RegistrationContext,
            event: &mut mod_api::RegisterParticleProvidersEvent<'_>,
        ) {
            assert!(self.capture_dropped.load(Relaxed));
            let binds = Arc::clone(&self.binds);
            event.register_sprite_set(Identifier::parse("example:peer").unwrap(), move |_| {
                binds.fetch_add(1, Relaxed);
                SpawnLightProbe::default()
            });
        }
    }

    let capture_dropped = Arc::new(AtomicBool::new(false));
    let binds = Arc::new(AtomicUsize::new(0));
    let mods = vec![
        mod_api::ModEntry::new(
            "first_refused",
            Box::new(Refused(Arc::clone(&capture_dropped))),
        ),
        mod_api::ModEntry::new(
            "later",
            Box::new(Later {
                capture_dropped,
                binds: Arc::clone(&binds),
            }),
        ),
    ];
    let (_, notices) = capture_callback_panic_notices(|| {
        mod_platform::testing::with_mods(mods.into(), || {
            let _phase = PhaseOwner::scope(ModPhase::Registration);
            let mut staged = Vec::new();
            mod_api::dispatch_register_particle_providers(&RegistrationContext::new(), &mut staged);
            let [registration] = staged.as_slice() else {
                panic!("only the later listener may publish a factory");
            };
            assert_eq!(registration.mod_id, "later");
            assert_eq!(binds.load(Relaxed), 0);
            let _engine = fixture_engine(Arc::new(native::freeze(staged)));
            assert_eq!(binds.load(Relaxed), 1);
        });
    });
    let [notice] = notices.as_slice() else {
        panic!("expected one toast, got {notices:?}");
    };
    assert_eq!(notice.mod_id, "first_refused");
    assert_eq!(notice.operation, "drop_particle_provider_factory");
    assert_eq!(notice.message, "staged factory capture drop");
}

#[test]
fn the_last_factory_binds_once_per_load_and_other_types_stay_declarative() {
    struct Drift(f64);
    impl ParticleProvider for Drift {
        fn create_particle(
            &self,
            spawn: &mut ParticleSpawn<'_>,
            particle: &mut dyn ParticleTickView,
        ) -> Option<Box<dyn CustomParticle>> {
            assert!(particle.set_sprite_index(spawn.velocity[0] as usize));
            Some(Box::new(Self(self.0)))
        }
    }
    impl CustomParticle for Drift {
        fn tick(&mut self, particle: &mut dyn ParticleTickView, _: &dyn ParticleLevelView) {
            if particle.advance_age() {
                let [_, y, z] = particle.position();
                particle.set_position([self.0, y, z]);
            }
        }
    }

    struct Register {
        position: u8,
        binds: Arc<Mutex<Vec<(u8, usize)>>>,
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Register {
        fn register_particle_providers(
            &self,
            _: &RegistrationContext,
            event: &mut mod_api::RegisterParticleProvidersEvent<'_>,
        ) {
            let (position, binds) = (self.position, self.binds.clone());
            event.register_sprite_set(Identifier::parse("mymod:orbit").unwrap(), move |sprites| {
                let frames = sprites.frame_count();
                binds.lock().unwrap().push((position, frames));
                if frames == 3 {
                    mod_api::__private::check_access(
                        "fixture_provider_world_access",
                        mod_api::__private::LoadAccess::WorldOnly,
                    );
                }
                Drift(f64::from(position))
            });
        }
    }

    let binds = Arc::new(Mutex::new(Vec::new()));
    let mods = [("first", 1), ("second", 2)].map(|(id, position)| {
        mod_api::ModEntry::new(
            id,
            Box::new(Register {
                position,
                binds: binds.clone(),
            }),
        )
    });
    mod_platform::testing::with_mods(Vec::from(mods).into(), || {
        let mut staged = Vec::new();
        mod_api::dispatch_register_particle_providers(&RegistrationContext::new(), &mut staged);
        let factories = Arc::new(native::freeze(staged));
        assert!(binds.lock().unwrap().is_empty());

        let root = |orbit: &[&str]| {
            ResourceSource::indexed(BTreeMap::from([
                (
                    "assets/mymod/particles/orbit.json".into(),
                    Resource::bytes(
                        serde_json::to_vec(&serde_json::json!({
                            "textures": orbit,
                            "sunrise:behaviour": {"lifetime": 7, "size": [0.3, 0.3]}
                        }))
                        .unwrap(),
                    ),
                ),
                (
                    "assets/mymod/particles/plain.json".into(),
                    Resource::bytes(
                        br#"{"textures":["mymod:one"],"sunrise:behaviour":{"lifetime":4}}"#
                            .to_vec(),
                    ),
                ),
            ]))
        };
        let sprite = Rect {
            u0: 0.0,
            u1: 0.25,
            v0: 0.0,
            v1: 0.25,
            w: 16,
            h: 16,
        };
        let atlas = HashMap::from([
            ("mymod:particle/one".into(), sprite),
            (
                MISSING_SPRITE.into(),
                Rect {
                    u0: 0.5,
                    u1: 0.75,
                    ..sprite
                },
            ),
        ]);
        let translucent = HashSet::new();
        let mut e = ParticleEngine::with_factories(
            &root(&["mymod:one", "mymod:two"]),
            &atlas,
            &translucent,
            &[],
            factories,
        );
        e.set_block_state_source(|| Some(Arc::new(Vec::<BlockState>::new())));
        assert_eq!(*binds.lock().unwrap(), [(2, 2)]);

        let scene = Scene::new();
        let level = scene.level();
        let options = fixture_options();
        let spawn = |e: &mut ParticleEngine, name: &str, frame: u8| {
            e.make(
                &ParticleOptions::named(name),
                [0.0; 3],
                [f64::from(frame), 0.0, 0.0],
                3,
                SpawnScope::with_level(&level, Some(&options)),
            )
        };
        let assert_sprite = |e: &mut ParticleEngine, frame, u0, u1| {
            let p = spawn(e, "mymod:orbit", frame).unwrap();
            e.push(p);
            e.tick(&level);
            let mut vertices = Vec::new();
            e.render(DVec3::ZERO, Quat::IDENTITY, 0.0, &|_| 15, &mut vertices);
            assert_eq!(vertices.len(), 4);
            assert!(vertices.iter().all(|v| (u0..=u1).contains(&v.uv[0])));
            assert!(vertices.iter().any(|v| v.uv[0] == u0));
            assert!(vertices.iter().any(|v| v.uv[0] == u1));
            e.clear();
        };
        assert_sprite(&mut e, 0, 0.0, 0.25);
        assert_sprite(&mut e, 1, 0.5, 0.75); // unresolved frame stays in its declared slot

        let mut orbit = spawn(&mut e, "mymod:orbit", 0).unwrap();
        assert_eq!((orbit.lifetime, orbit.size), (7, 0.3));
        e.tick_particle(&mut orbit, &level);
        assert_eq!(orbit.position[0], 2.0);
        assert_eq!(*binds.lock().unwrap(), [(2, 2)]);
        e.push(orbit);

        use mod_api::__private::{ModPhase, PhaseOwner};
        let _phase = PhaseOwner::scope(ModPhase::Reloading);
        e.reload(
            &root(&["mymod:one", "mymod:two", "mymod:three"]),
            &atlas,
            &translucent,
        );
        assert!(e.pending.is_empty());
        assert_eq!(*binds.lock().unwrap(), [(2, 2), (2, 3)]);
        assert!(spawn(&mut e, "mymod:orbit", 0).is_none()); // refused; no declarative fallback
        let plain = spawn(&mut e, "mymod:plain", 0).unwrap();
        assert_eq!(plain.lifetime, 4);
        assert!(plain.provided.is_none() && matches!(plain.behaviour, Behaviour::Mod { .. }));

        e.reload(&root(&["mymod:one"]), &atlas, &translucent);
        assert_eq!(*binds.lock().unwrap(), [(2, 2), (2, 3), (2, 1)]);
        assert!(spawn(&mut e, "mymod:orbit", 0).unwrap().provided.is_some());

        e.reload(&root(&[]), &atlas, &translucent);
        assert_eq!(*binds.lock().unwrap(), [(2, 2), (2, 3), (2, 1), (2, 1)]);
        assert_sprite(&mut e, 0, 0.5, 0.75); // valid empty description uses one missing sprite

        e.reload(
            &ResourceSource::indexed(Default::default()),
            &atlas,
            &translucent,
        );
        assert_eq!(*binds.lock().unwrap(), [(2, 2), (2, 3), (2, 1), (2, 1)]);
        assert!(spawn(&mut e, "mymod:orbit", 0).is_none()); // no description, no bind
    });
}

#[test]
fn a_polytone_definition_overrides_a_bound_native_provider() {
    struct Probe;
    impl ParticleProvider for Probe {
        fn create_particle(
            &self,
            _: &mut ParticleSpawn<'_>,
            particle: &mut dyn ParticleTickView,
        ) -> Option<Box<dyn CustomParticle>> {
            particle.set_lifetime(7);
            Some(Box::new(Self))
        }
    }
    impl CustomParticle for Probe {}

    let mut engine = fixture_engine(provider_factories([("example:orbit", Arc::new(Probe))]));
    let scene = Scene::new();
    let level = scene.level();
    let options = fixture_options();
    let spawn = |e: &mut ParticleEngine| {
        e.add_with(
            &ParticleOptions::named("example:orbit"),
            [0.0; 3],
            [0.0; 3],
            ParticleSpawnFlags::default(),
            SpawnScope::with_level(&level, Some(&options)),
        );
        e.tick(&level);
        e.live().next().unwrap().lifetime
    };
    assert_eq!(spawn(&mut engine), 7, "native provider bound successfully");
    engine.clear();

    let pack = ResourceSource::indexed(
        [(
            "assets/example/polytone/custom_particles/orbit.json".into(),
            Resource::bytes(br#"{"has_physics":false,"initializer":{"lifetime":"17"}}"#.to_vec()),
        )]
        .into(),
    );
    engine.set_polytone(Some(Arc::new(polytone::Registry::read(
        &pack,
        Default::default(),
        scene.collision.registry(),
    ))));
    assert_eq!(spawn(&mut engine), 17, "the selected pack type wins");
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
