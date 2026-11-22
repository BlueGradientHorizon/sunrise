//! Headless proof of the generated Host bridge and the ordinary particle queue.
use super::{
    Game, execution_role::ExecutionRole, native_client_particles::configure_particle_inputs,
};
use mod_api::ClientHost;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};
use sunrise_client::{
    Result,
    hud::Rect,
    motion::PlayerMotion,
    particles::{ParticleEngine, ParticleLevel, ParticleOptions, ParticleType},
};
use sunrise_core::geometry::Vec3;

fn add(game: &mut Game<'_>, options: &ParticleOptions, at: [f64; 3], velocity: [f64; 3]) -> bool {
    use super::api_values::TryIntoApi;
    let Ok(options) = options.try_into_api() else {
        return false;
    };
    let host: &mut dyn ClientHost = game;
    host.add_particle(&options, at, velocity)
}

impl Game<'_> {
    fn particle_fixture_tick(&mut self) {
        let level = ParticleLevel {
            world: &self.world,
            facts: self.res.facts(),
            collision: self.collision,
            player: None,
            cached_chunk: std::cell::Cell::new(None),
        };
        self.particles.tick(&level);
    }

    pub(super) fn run_native_client_particles_fixture(&mut self, directory: &Path) -> Result<()> {
        // The fixture stands in for a joined client level, where the mods' calls run in the
        // World phase; the loading window it starts from is still in Setup.
        let _world = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let at = [0.0, 100.0, 0.0];
        let smoke = ParticleOptions::simple(ParticleType::Smoke);
        self.particles.requested = Some(Vec::new());
        self.player = None;
        assert!(!add(self, &smoke, at, [0.0; 3]), "menu has no client level");
        self.player = Some(PlayerMotion::new(self.motion, Vec3::new(0.0, 100.0, 0.0))?);
        for role in [
            ExecutionRole::IntegratedServer,
            ExecutionRole::DedicatedServer,
        ] {
            self.options.execution_role = role;
            assert!(
                !add(self, &smoke, at, [0.0; 3]),
                "servers cannot spawn client particles"
            );
        }
        self.options.execution_role = ExecutionRole::Client;
        let encoded = [f64::from(0xff12ab), 7.0, -2.0];
        let mut custom = ParticleOptions::named("fixture:encoded");
        custom.color = [0.1, 0.2, 0.3, 0.4];
        custom.scale = 2.5;
        assert!(add(self, &custom, at, encoded));
        let requests = self.particles.requested.take().unwrap();
        assert_eq!(requests.len(), 1, "menu and server guards queued nothing");
        assert_eq!(requests[0].0.color, custom.color);
        assert_eq!(requests[0].0.scale, custom.scale);
        assert_eq!(
            requests[0].0.mod_particle.as_deref(),
            Some("fixture:encoded")
        );
        assert_eq!(
            (requests[0].1, requests[0].2),
            (at, encoded),
            "provider slots must remain unchanged"
        );
        self.particles.requested = Some(Vec::new());
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(!add(self, &smoke, [invalid, 100.0, 0.0], [0.0; 3]));
            assert!(!add(self, &smoke, at, [0.0, invalid, 0.0]));
        }
        let mut invalid_options = smoke.clone();
        invalid_options.color[1] = f32::NAN;
        assert!(!add(self, &invalid_options, at, [0.0; 3]));
        invalid_options = smoke.clone();
        invalid_options.scale = f32::INFINITY;
        assert!(!add(self, &invalid_options, at, [0.0; 3]));

        assert!(
            self.particles.requested.take().unwrap().is_empty(),
            "invalid requests must not touch the queue"
        );

        // Own sprite metadata and atlas rectangles exercise real constructors and CPU quads,
        // without a GPU, downloaded art, a resource reload or a change to imported resources.
        let pack = directory.join("particle-pack");
        std::fs::create_dir_all(pack.join("assets/minecraft/particles"))?;
        std::fs::create_dir_all(pack.join("assets/fixture/particles"))?;
        std::fs::write(
            pack.join("assets/minecraft/particles/smoke.json"),
            br#"{"textures":["fixture_smoke"]}"#,
        )?;
        std::fs::write(pack.join("assets/fixture/particles/spark.json"), br#"{"textures":["fixture:spark"],"sunrise:behaviour":{"velocity":[0.25,0,0],"gravity":0,"friction":1,"lifetime":3,"size":[0.2,0.2],"animation":"first"}}"#)?;
        let rect = Rect {
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 1.0,
            w: 16,
            h: 16,
        };
        self.particles = ParticleEngine::new(
            &sunrise_assets::resource_source::ResourceSource::directory(pack.clone()),
            &HashMap::from([
                ("particle/fixture_smoke".into(), rect),
                ("fixture:particle/spark".into(), rect),
            ]),
            &HashSet::new(),
        );
        configure_particle_inputs(&mut self.particles);
        self.particles.camera = at;
        assert!(add(self, &smoke, at, [0.0; 3]));
        assert_eq!(
            self.particles.len(),
            0,
            "engine defers pending spawns until its tick"
        );
        self.particle_fixture_tick();
        assert_eq!(
            self.particles.len(),
            1,
            "vanilla provider reached the live queue"
        );
        let mut vertices = Vec::new();
        self.particles.render(
            glam::DVec3::from_array(at),
            glam::Quat::IDENTITY,
            1.0,
            &|_| 15,
            &mut vertices,
        );
        assert_eq!(
            vertices.len(),
            4,
            "vanilla particle reaches ordinary render data"
        );
        self.particles.clear();
        assert!(add(self, &smoke, [64.0, 100.0, 0.0], [0.0; 3]));
        self.particle_fixture_tick();
        assert!(
            self.particles.is_empty(),
            "ordinary camera-distance limiter is retained"
        );
        self.particles.particle_level = 2;
        assert!(add(self, &smoke, at, [0.0; 3]));
        self.particle_fixture_tick();
        assert!(
            self.particles.is_empty(),
            "ordinary minimal setting is retained"
        );
        self.particles.particle_level = 0;
        assert!(add(
            self,
            &ParticleOptions::named("fixture:spark"),
            at,
            [0.0; 3]
        ));
        self.particle_fixture_tick();
        assert_eq!(
            self.particles.len(),
            1,
            "existing named provider reached the live queue"
        );
        self.particle_fixture_tick();
        vertices.clear();
        self.particles.render(
            glam::DVec3::from_array(at),
            glam::Quat::IDENTITY,
            1.0,
            &|_| 15,
            &mut vertices,
        );
        assert_eq!(vertices.len(), 4);
        assert!(vertices.iter().flat_map(|v| v.pos).all(f32::is_finite));
        let center_x = vertices.iter().map(|v| v.pos[0]).sum::<f32>() / 4.0;
        assert!(
            (center_x - 0.25).abs() < 1e-6,
            "named behavior moved the real particle: {center_x}"
        );
        for _ in 0..10 {
            self.particle_fixture_tick();
        }
        assert!(
            self.particles.is_empty(),
            "named provider lifetime still expires normally"
        );
        std::fs::write(
            directory.join("report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "client_spawn": true, "encoded_slots_unchanged": true,
                "menu_and_servers_refused": true, "nonfinite_refused": true,
                "vanilla_live_particles": 1, "vanilla_vertices": 4,
                "named_live_particles": 1, "named_vertices": 4, "named_tick_x": center_x,
                "named_expired": true, "normal_limiters_retained": true,
                "scope": "Generated Host bridge into existing particle queue, providers, tick and CPU render data; no GPU/window or new providers/triggers."
            }))?,
        )?;
        Ok(())
    }
}

// The standalone route fixture uses content-only assets. Exercise its native provider counterpart
// in-process, so the trait mod and the factory table are scoped, not installed in the live game.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{
        api_values::TryIntoApi,
        mods::resolve_particle_providers,
        native_entity_refs::tests::{run_with_game_stack, with_client_game},
    };
    use mod_api::{
        __private::{ModPhase, PhaseOwner, game::RegistrationContextNew},
        BlockState, CustomParticle, Identifier, ModEntry, ParticleProvider, ParticleSpawn,
        ParticleTickView, RegisterParticleProvidersEvent, Registrar, RegistrationContext,
    };
    use std::sync::{Arc, Mutex};
    use sunrise_assets::resource_source::{Resource, ResourceSource};
    use sunrise_client::chunk::Chunk;
    use sunrise_protocol::{
        entity_data::{MOD_PARTICLE_ID, Particle, ParticleData},
        packets::play::LevelParticles,
    };

    const TYPE: &str = "fixture:encoded";
    const BLOCK: [i32; 3] = [5, 100, 4];

    #[derive(Debug, PartialEq)]
    struct ObservedSpawn {
        options: mod_api::ParticleOptions,
        position: [f64; 3],
        velocity: [f64; 3],
        block: BlockState,
    }

    #[derive(Clone)]
    struct Provider(Arc<Mutex<Vec<ObservedSpawn>>>);

    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Provider {
        fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
            registrar.particle_key(TYPE);
        }

        fn register_particle_providers(
            &self,
            _: &RegistrationContext,
            event: &mut RegisterParticleProvidersEvent<'_>,
        ) {
            let provider = self.clone();
            event.register_sprite_set(Identifier::parse(TYPE).unwrap(), move |_| provider.clone());
        }
    }

    impl ParticleProvider for Provider {
        fn create_particle(
            &self,
            spawn: &mut ParticleSpawn<'_>,
            _: &mut dyn ParticleTickView,
        ) -> Option<Box<dyn CustomParticle>> {
            struct Instance;
            impl CustomParticle for Instance {}
            self.0.lock().unwrap().push(ObservedSpawn {
                options: spawn.options.clone(),
                position: spawn.position,
                velocity: spawn.velocity,
                block: spawn.level.block_state(BLOCK),
            });
            Some(Box::new(Instance))
        }
    }

    #[test]
    #[ignore = "daily"]
    fn registered_provider_receives_host_and_network_spawns_with_live_level() {
        run_with_game_stack(|| {
            mod_platform::testing::with_mods(Default::default(), || {
                with_client_game(|game| {
                    let registry = game.pack.assets.registry;
                    let stone = registry.default_state("minecraft:stone").unwrap();
                    let dirt = registry.default_state("minecraft:dirt").unwrap();
                    let stone_api = stone.try_into_api().unwrap();
                    let dirt_api = dirt.try_into_api().unwrap();
                    let [x, y, z] = BLOCK;
                    game.world
                        .chunks
                        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                    game.world.set(x, y, z, stone).unwrap();

                    let observed = Arc::new(Mutex::new(Vec::new()));
                    let mods = vec![ModEntry::new(
                        "fixture",
                        Box::new(Provider(observed.clone())),
                    )];
                    mod_platform::testing::with_mods(mods.into(), || {
                        // Resolve this fixture's particle options from its own Parts epoch and
                        // sprite declaration, keeping the loaded game's block state index.
                        mod_platform::__private::part_tests::install_parts_fixture(
                            mod_api::__private::mods(),
                        )
                        .unwrap();
                        let source = ResourceSource::indexed(
                            [(
                                "assets/fixture/particles/encoded.json".into(),
                                Resource::bytes(br#"{"textures":["fixture:encoded"]}"#.to_vec()),
                            )]
                            .into(),
                        );
                        resolve_particle_providers(&source).unwrap();
                        let ty = mod_api::__private::particle_named_key(TYPE).unwrap();
                        let mut factories = Vec::new();
                        Game::native_register_particle_providers(
                            &RegistrationContext::new(),
                            &mut factories,
                        );
                        game.particles = ParticleEngine::with_test_providers(
                            &source,
                            &HashMap::from([(
                                "fixture:particle/encoded".into(),
                                Rect {
                                    u0: 0.0,
                                    v0: 0.0,
                                    u1: 1.0,
                                    v1: 1.0,
                                    w: 16,
                                    h: 16,
                                },
                            )]),
                            factories,
                        );
                        configure_particle_inputs(&mut game.particles);

                        let at = [5.5, 101.0, 4.5];
                        let velocity = [f64::from(0xff12ab), 7.0, -2.0];
                        let mut options =
                            mod_api::ParticleOptions::colored(ty, [0.1, 0.2, 0.3, 0.4]);
                        options.scale = 2.5;
                        game.player = Some(
                            PlayerMotion::new(game.motion, Vec3::new(at[0], at[1], at[2])).unwrap(),
                        );
                        game.particles.camera = at;
                        let _world = PhaseOwner::scope(ModPhase::World);
                        let host: &mut dyn ClientHost = game;
                        assert!(host.add_particle(&options, at, velocity));

                        // The next construction must see this change through the same level loan
                        // path, even though the options arrived via a server packet this time.
                        game.world.set(x, y, z, dirt).unwrap();
                        let packet = LevelParticles {
                            particle: Particle {
                                id: MOD_PARTICLE_ID,
                                data: Some(ParticleData::Mod(TYPE.into())),
                            },
                            override_limiter: false,
                            always_show: false,
                            pos: [6.5, 101.25, 4.5],
                            dist: [0.5, -2.0, 4.0],
                            max_speed: [4.0, 3.0, 0.25],
                            count: 0,
                            randomization: 0,
                        };
                        game.remote_level_particles(&packet);

                        let far = [64.0, 101.0, 4.5];
                        let host: &mut dyn ClientHost = game;
                        assert!(host.add_particle(&options, far, velocity));
                        game.remote_level_particles(&LevelParticles {
                            pos: far,
                            ..packet.clone()
                        });
                        game.particles.particle_level = 2;
                        let host: &mut dyn ClientHost = game;
                        assert!(host.add_particle(&options, at, velocity));
                        game.remote_level_particles(&packet);

                        assert_eq!(
                            *observed.lock().unwrap(),
                            [
                                ObservedSpawn {
                                    options,
                                    position: at,
                                    velocity,
                                    block: stone_api
                                },
                                ObservedSpawn {
                                    options: mod_api::ParticleOptions::simple(ty),
                                    position: [6.5, 101.25, 4.5],
                                    velocity: [2.0, -6.0, 1.0],
                                    block: dirt_api,
                                },
                            ],
                            "only admitted requests construct, with their original typed options and the live level",
                        );
                        assert!(game.particles.is_empty());
                        game.particle_fixture_tick();
                        assert_eq!(game.particles.len(), 2);
                    });
                });
            });
        });
    }
}
