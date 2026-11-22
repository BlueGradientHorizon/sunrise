use super::super::*;
use crate::chunk::{Chunk, Light};
use mod_api::{
    __private::game::RegistrationContextNew, BlockState, Identifier, LightLayer, ModParticle,
    ParticleBatch, ParticleFacing, ParticleLevelView, ParticleProvider, RegistrationContext,
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
/// A scoped registration for fixtures that exercise a provider's existing batch behavior.
pub(in crate::particles) fn provider_factories(
    id: &str,
    provider: Arc<dyn ParticleProvider>,
) -> Arc<native::Factories> {
    struct Shared(Arc<dyn ParticleProvider>);
    impl ParticleProvider for Shared {
        fn tick(&self, particles: &mut ParticleBatch, level: &dyn ParticleLevelView) {
            self.0.tick(particles, level);
        }
    }
    struct Register(Identifier, Arc<dyn ParticleProvider>);
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Register {
        fn register_particle_providers(
            &self,
            _: &RegistrationContext,
            event: &mut mod_api::RegisterParticleProvidersEvent<'_>,
        ) {
            let provider = self.1.clone();
            event.register_sprite_set(self.0.clone(), move |_| Shared(provider.clone()));
        }
    }

    let id = Identifier::parse(id).unwrap();
    let mods = vec![mod_api::ModEntry::new(
        "provider_fixture",
        Box::new(Register(id, provider)),
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

fn fixture_engine(factories: Arc<native::Factories>) -> ParticleEngine {
    let root = ResourceSource::indexed(BTreeMap::from([(
        "assets/example/particles/orbit.json".into(),
        Resource::bytes(br#"{"textures":["example:one","example:two"]}"#.to_vec()),
    )]));
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
    let mut e = ParticleEngine::with_factories(&root, &atlas, &HashSet::new(), &[], factories);
    // The game's table of block states; the providers here read none of them.
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
/// One engine tick of a single provider particle: the age step, then the provider's batch.
fn tick_one(e: &mut ParticleEngine, p: &mut Particle, level: &ParticleLevel<'_>) {
    e.tick_particle(p, level);
    e.tick_providers(std::slice::from_mut(p), level);
}
fn engine_with(provider: Arc<dyn ParticleProvider>) -> ParticleEngine {
    fixture_engine(provider_factories("example:orbit", provider))
}
fn provided(e: &mut ParticleEngine) -> Particle {
    e.make(
        &ParticleOptions::named("example:orbit"),
        [1.0; 3],
        [0.0; 3],
        5,
        SpawnScope::BARE,
    )
    .unwrap()
}
/// A particle a provider poses on each of its ticks (`rolls[age - 1]`), drawn half a tick
/// into the last one with the camera turned by `base`, as the renderer builds its corners.
fn drawn(
    facing: ParticleFacing,
    rolls: &[f32],
    center: [f64; 3],
    base: glam::Quat,
) -> Vec<[f32; 3]> {
    struct Pose(ParticleFacing, Vec<f32>, [f64; 3]);
    impl ParticleProvider for Pose {
        fn tick(&self, particles: &mut ParticleBatch, _: &dyn ParticleLevelView) {
            for particle in particles {
                particle.position = self.2;
                particle.previous = self.2;
                particle.size = 1.0;
                particle.facing = self.0;
                particle.roll = self.1[particle.age as usize - 1];
            }
        }
    }
    let scene = Scene::new();
    let level = scene.level();
    let mut e = engine_with(Arc::new(Pose(facing, rolls.to_vec(), center)));
    let mut p = provided(&mut e);
    p.lifetime = 20;
    for _ in rolls {
        tick_one(&mut e, &mut p, &level);
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
fn lit_chunk() -> Chunk {
    let mut chunk = Chunk::empty(0, 0);
    let section = chunk.section_index(64).unwrap();
    Arc::make_mut(&mut chunk.sections[section]).light = Light::packed(Box::new([0xB4; 4096]));
    chunk
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
fn the_last_factory_binds_once_per_load_and_other_types_stay_declarative() {
    struct Drift(f64);
    impl ParticleProvider for Drift {
        fn tick(&self, particles: &mut ParticleBatch, _: &dyn ParticleLevelView) {
            for particle in particles {
                particle.position[0] = self.0;
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
        let atlas = HashMap::new();
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
        let spawn = |e: &mut ParticleEngine, name: &str| {
            e.make(
                &ParticleOptions::named(name),
                [0.0; 3],
                [0.0; 3],
                3,
                SpawnScope::BARE,
            )
        };
        let mut orbit = spawn(&mut e, "mymod:orbit").unwrap();
        assert_eq!((orbit.lifetime, orbit.size), (7, 0.3));
        tick_one(&mut e, &mut orbit, &level);
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
        assert!(spawn(&mut e, "mymod:orbit").is_none()); // refused; no declarative fallback
        let plain = spawn(&mut e, "mymod:plain").unwrap();
        assert_eq!(plain.lifetime, 4);
        assert!(plain.provided.is_none() && matches!(plain.behaviour, Behaviour::Mod { .. }));

        e.reload(&root(&["mymod:one"]), &atlas, &translucent);
        assert_eq!(*binds.lock().unwrap(), [(2, 2), (2, 3), (2, 1)]);
        assert!(spawn(&mut e, "mymod:orbit").unwrap().provided.is_some());

        e.reload(&root(&[]), &atlas, &translucent);
        assert_eq!(*binds.lock().unwrap(), [(2, 2), (2, 3), (2, 1)]);
        assert!(spawn(&mut e, "mymod:orbit").is_none()); // missing set; binder not called
    });
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
