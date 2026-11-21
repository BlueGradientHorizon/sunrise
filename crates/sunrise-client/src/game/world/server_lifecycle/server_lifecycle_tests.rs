use crate::game::{
    Dim, execution_role::ExecutionRole, native_entity_refs::tests::with_game,
    native_guest_turn_tests::with_guests,
};
use mod_api::{Hooks, ModEntry, ServerHost};
use mod_platform::testing::with_mods;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct PanicAt {
    stopping: AtomicBool,
    save: AtomicBool,
    config_unloading: AtomicBool,
    logout: AtomicBool,
}

struct StoppedProbe {
    serving_stopped: Box<dyn Fn() -> bool + Send + Sync>,
    lan: std::sync::Weak<sunrise_client::lan::Shared>,
    root: std::path::PathBuf,
    save: Arc<sunrise_client::save::SaveDir>,
    expected_last: &'static str,
}

/// Exercise the real stop/save paths, not just direct calls to hook dispatch.
#[test]
#[ignore = "daily"]
fn every_started_server_stops_and_saves_even_when_startup_or_ticks_fail() {
    struct Log(
        Arc<Mutex<Vec<String>>>,
        Arc<PanicAt>,
        Arc<Mutex<Option<StoppedProbe>>>,
    );
    #[mod_api::__subscriptions]
    impl Hooks for Log {
        fn register_configs(
            &self,
            _: mod_api::HostSide,
            e: &mut mod_api::RegisterConfigsEvent<'_>,
        ) {
            e.configs
                .push(mod_api::Config::new("log", mod_api::ConfigKind::Server, ""));
        }
        fn server_about_to_start(
            &self,
            _: &mut dyn ServerHost,
            _: &mut mod_api::ServerAboutToStartEvent,
        ) {
            self.0.lock().unwrap().push("about-to-start".into());
        }
        fn level_load(&self, _: &mut dyn ServerHost, e: &mut mod_api::LevelLoadEvent<'_>) {
            self.0
                .lock()
                .unwrap()
                .push(format!("level-load:{}", e.dimension));
        }
        fn server_starting(&self, _: &mut dyn ServerHost, _: &mut mod_api::ServerStartingEvent) {
            self.0.lock().unwrap().push("starting".into());
        }
        fn server_started(&self, _: &mut dyn ServerHost, _: &mut mod_api::ServerStartedEvent) {
            self.0.lock().unwrap().push("started".into());
        }
        fn server_stopping(&self, _: &mut dyn ServerHost, _: &mut mod_api::ServerStoppingEvent) {
            self.0.lock().unwrap().push("stopping".into());
            if self.1.stopping.load(Ordering::Relaxed) {
                panic!("stopping listener");
            }
        }
        fn config_unloading(
            &self,
            _: &mut dyn ServerHost,
            _: &mut mod_api::ConfigUnloadingEvent<'_>,
        ) {
            self.0.lock().unwrap().push("config-unloading".into());
            if self.1.config_unloading.load(Ordering::Relaxed) {
                panic!("config unloading listener");
            }
        }
        fn level_save(&self, _: &mut dyn ServerHost, e: &mut mod_api::LevelSaveEvent<'_>) {
            self.0
                .lock()
                .unwrap()
                .push(format!("level-save:{}", e.dimension));
        }
        fn world_saved(&self, _: &mut dyn ServerHost, _: &mut mod_api::WorldSavedEvent) {
            self.0.lock().unwrap().push("saved".into());
            if self.1.save.load(Ordering::Relaxed) {
                panic!("save listener");
            }
        }
        fn player_logged_out(
            &self,
            _: &mut dyn ServerHost,
            _: &mut mod_api::PlayerLoggedOutEvent<'_>,
        ) {
            if self.1.logout.load(Ordering::Relaxed) {
                self.0.lock().unwrap().push("logout".into());
                panic!("logout listener");
            }
        }
        fn level_unload(&self, _: &mut dyn ServerHost, e: &mut mod_api::LevelUnloadEvent<'_>) {
            self.0
                .lock()
                .unwrap()
                .push(format!("level-unload:{}", e.dimension));
        }
        fn server_stopped(&self, _: &mut dyn ServerHost, _: &mut mod_api::ServerStoppedEvent) {
            if let Some(probe) = self.2.lock().unwrap().as_ref() {
                assert!((probe.serving_stopped)(), "serving retired before Stopped");
                assert!(probe.lan.upgrade().is_none(), "LAN dropped before Stopped");
                let saved = sunrise_client::save::Level::read(&probe.root).unwrap();
                assert_eq!(saved.name, "lifecycle final save");
                assert!(
                    probe.save.raw_chunk(0, 0).is_some(),
                    "dirty chunk saved before Stopped"
                );
                assert_eq!(
                    self.0.lock().unwrap().last().map(String::as_str),
                    Some(probe.expected_last)
                );
                // Storage is released before the notification, even if the
                // final-save listener itself panicked after writing metadata.
                let _lock = sunrise_client::save::SessionLock::acquire(&probe.root).unwrap();
            }
            self.0.lock().unwrap().push("stopped".into());
        }
    }
    let log = Arc::new(Mutex::new(Vec::new()));
    let panic_at = Arc::new(PanicAt {
        stopping: AtomicBool::new(false),
        save: AtomicBool::new(false),
        config_unloading: AtomicBool::new(false),
        logout: AtomicBool::new(false),
    });
    let stopped_probe = Arc::new(Mutex::new(None));
    with_mods(
        vec![ModEntry::new(
            "log",
            Box::new(Log(log.clone(), panic_at.clone(), stopped_probe.clone())),
        )]
        .into(),
        || {
            for dedicated in [false, true] {
                for case in [
                    "normal",
                    "startup-error",
                    "opening-early-ok",
                    "bind-failed-early-ok",
                    "runtime-extra-level",
                    "tick-error",
                    "panic",
                    "save-error",
                    "unopened",
                    "locked-unadopted",
                    "stopping-panic",
                    "save-panic",
                    "config-unloading-panic",
                    "config-unloading-save-panic",
                    "config-unloading-logout-panic",
                ] {
                    log.lock().unwrap().clear();
                    panic_at
                        .stopping
                        .store(case == "stopping-panic", Ordering::Relaxed);
                    panic_at.save.store(
                        matches!(case, "save-panic" | "config-unloading-save-panic"),
                        Ordering::Relaxed,
                    );
                    panic_at
                        .logout
                        .store(case == "config-unloading-logout-panic", Ordering::Relaxed);
                    let config_panic = case.starts_with("config-unloading");
                    panic_at
                        .config_unloading
                        .store(config_panic, Ordering::Relaxed);
                    *stopped_probe.lock().unwrap() = None;
                    let run_case = |game: &mut crate::game::Game<'_>| {
                        game.options.execution_role = if dedicated {
                            ExecutionRole::DedicatedServer
                        } else {
                            ExecutionRole::IntegratedServer
                        };
                        let root = std::env::temp_dir().join(format!(
                            "server-lifecycle-{}-{dedicated}-{case}",
                            std::process::id()
                        ));
                        std::fs::create_dir_all(&root).unwrap();
                        let serverconfig = root.join("serverconfig");
                        std::fs::create_dir_all(&serverconfig).unwrap();
                        std::fs::write(serverconfig.join("log"), "").unwrap();
                        game.world_root = root.clone();
                        game.save = Arc::new(sunrise_client::save::SaveDir::new(
                            root.clone(),
                            sunrise_client::save::LevelShape::OVERWORLD,
                            Some(Default::default()),
                        ));
                        game.level.name = "lifecycle final save".into();
                        let adopted = !matches!(
                            case,
                            "unopened" | "bind-failed-early-ok" | "locked-unadopted"
                        );
                        if adopted || case == "locked-unadopted" {
                            game.session_lock =
                                Some(sunrise_client::save::SessionLock::acquire(&root).unwrap());
                        }
                        // The lock is held while tags are readable, before the prepared
                        // level replaces the placeholder. Only adoption may save.
                        game.world_adopted = adopted;
                        if matches!(
                            case,
                            "unopened" | "bind-failed-early-ok" | "locked-unadopted"
                        ) {
                            // A failed open must not overwrite an existing, unreadable save.
                            std::fs::write(root.join("level.dat"), b"unadopted save").unwrap();
                        }
                        if config_panic {
                            game.level.name = "previous save".into();
                            game.level.write(&root, &Default::default()).unwrap();
                            // Nothing queues these metadata/block changes. A flush
                            // without the final save must leave the old name and no chunk.
                            game.level.name = "lifecycle final save".into();
                            game.world.chunks.insert(
                                (0, 0),
                                Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
                            );
                            let stone = game
                                .pack
                                .assets
                                .registry
                                .default_state("minecraft:stone")
                                .unwrap();
                            game.world.set(1, 64, 1, stone).unwrap();
                            assert!(game.save.raw_chunk(0, 0).is_none());
                            let serving_stopped = if dedicated {
                                Box::new(|| true) as Box<dyn Fn() -> bool + Send + Sync>
                            } else {
                                // Cover both a published and an unpublished integrated
                                // server. Neither may merely unpublish on shutdown.
                                Box::new(super::super::integrated::tests::install_serving(
                                    game,
                                    case == "config-unloading-save-panic",
                                ))
                            };
                            let lan = Arc::downgrade(&game.lan.as_ref().unwrap().hub.lan.shared);
                            *stopped_probe.lock().unwrap() = Some(StoppedProbe {
                                serving_stopped,
                                lan,
                                root: root.clone(),
                                save: game.save.clone(),
                                expected_last: if case == "config-unloading-save-panic" {
                                    "saved"
                                } else {
                                    "level-unload:minecraft:overworld"
                                },
                            });
                        }
                        let result = game.run_server_lifecycle(dedicated, |game| {
                            if case == "bind-failed-early-ok" {
                                return Ok(super::ServerRunOutcome::FailedInit);
                            }
                            game.native_configs_server_start(&root, dedicated);
                            game.native_server_about_to_start(dedicated);
                            game.native_lifecycle_open_started();
                            if case == "unopened" {
                                return Err("failed init".into());
                            }
                            game.native_level_opened(game.dim);
                            if matches!(case, "startup-error" | "locked-unadopted") {
                                return Err("failed init".into());
                            }
                            if case == "opening-early-ok" {
                                return Ok(super::ServerRunOutcome::FailedInit);
                            }
                            game.native_server_starting(dedicated);
                            game.native_lifecycle_world_ready();
                            if case == "runtime-extra-level" {
                                game.join_world_rest().unwrap();
                                Dim::add_extra("test:pocket");
                                let pocket = Dim::extra("test:pocket").unwrap();
                                let parked = game.open_level(pocket).unwrap();
                                game.levels.parked.insert(pocket, parked);
                                game.native_level_opened(pocket);
                            }
                            match case {
                                "tick-error" => Err("failed tick".into()),
                                "panic" => panic!("failed tick"),
                                "save-error" => {
                                    let file = root.join("not-a-directory");
                                    std::fs::write(&file, b"occupied").unwrap();
                                    game.world_root = file;
                                    Ok(super::ServerRunOutcome::Orderly)
                                }
                                _ => Ok(super::ServerRunOutcome::Orderly),
                            }
                        });
                        assert_eq!(
                            result.is_ok(),
                            matches!(
                                case,
                                "normal"
                                    | "opening-early-ok"
                                    | "bind-failed-early-ok"
                                    | "runtime-extra-level"
                            ),
                            "{dedicated} {case}"
                        );
                        if config_panic || matches!(case, "stopping-panic" | "save-panic") {
                            let message = result.unwrap_err().to_string();
                            let needle = if config_panic {
                                "config unloading listener"
                            } else if case == "stopping-panic" {
                                "stopping listener"
                            } else {
                                "save listener"
                            };
                            assert!(message.contains(needle), "{dedicated} {case}: {message}");
                            if config_panic {
                                assert_eq!(
                                    message,
                                    "the server shutdown panicked: config unloading listener"
                                );
                            }
                        }
                        let expected: &[&str] = match case {
                            "bind-failed-early-ok" => &["stopped"],
                            "unopened" => &["about-to-start", "config-unloading", "stopped"],
                            "locked-unadopted" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "config-unloading",
                                "level-unload:minecraft:overworld",
                                "stopped",
                            ],
                            "startup-error" | "opening-early-ok" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "config-unloading",
                                "level-save:minecraft:overworld",
                                "saved",
                                "level-unload:minecraft:overworld",
                                "stopped",
                            ],
                            "tick-error" | "panic" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "config-unloading",
                                "level-save:minecraft:overworld",
                                "saved",
                                "level-unload:minecraft:overworld",
                                "stopped",
                            ],
                            "save-error" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "stopping",
                                "config-unloading",
                                "level-save:minecraft:overworld",
                                "stopped",
                            ],
                            "save-panic" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "stopping",
                                "config-unloading",
                                "level-save:minecraft:overworld",
                                "saved",
                                "stopped",
                            ],
                            "runtime-extra-level" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "level-load:test:pocket",
                                "stopping",
                                "config-unloading",
                                "level-save:minecraft:overworld",
                                "level-save:test:pocket",
                                "saved",
                                "level-unload:minecraft:overworld",
                                "level-unload:test:pocket",
                                "stopped",
                            ],
                            "config-unloading-logout-panic" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "stopping",
                                "config-unloading",
                                "logout",
                                "level-save:minecraft:overworld",
                                "saved",
                                "level-unload:minecraft:overworld",
                                "stopped",
                            ],
                            "config-unloading-save-panic" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "stopping",
                                "config-unloading",
                                "level-save:minecraft:overworld",
                                "saved",
                                "stopped",
                            ],
                            _ => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "stopping",
                                "config-unloading",
                                "level-save:minecraft:overworld",
                                "saved",
                                "level-unload:minecraft:overworld",
                                "stopped",
                            ],
                        };
                        assert_eq!(*log.lock().unwrap(), expected, "{dedicated} {case}");
                        game.native_lifecycle_world_closed();
                        assert_eq!(
                            *log.lock().unwrap(),
                            expected,
                            "second close is a no-op after {case}"
                        );
                        assert!(game.session_lock.is_none(), "storage released after {case}");
                        assert!(!game.world_adopted, "adoption cleared after {case}");
                        assert!(game.serving.is_none(), "serving retired after {case}");
                        assert!(game.lan.is_none(), "LAN dropped after {case}");
                        if config_panic {
                            let registry = &game.pack.assets.registry;
                            let chunk =
                                game.save.load(0, 0, registry, world_biomes!(game)).unwrap();
                            assert_eq!(
                                chunk.block(1, 64, 1),
                                registry.default_state("minecraft:stone").unwrap()
                            );
                        }
                        if matches!(
                            case,
                            "unopened" | "bind-failed-early-ok" | "locked-unadopted"
                        ) {
                            assert_eq!(
                                std::fs::read(root.join("level.dat")).unwrap(),
                                b"unadopted save",
                                "{dedicated} {case}"
                            );
                        } else if case != "save-error" {
                            let saved = sunrise_client::save::Level::read(&root).unwrap();
                            assert_eq!(saved.name, "lifecycle final save");
                        }
                        if case == "runtime-extra-level" {
                            Dim::set_extra(std::iter::empty());
                        }
                        std::fs::remove_dir_all(root).unwrap();
                    };
                    if config_panic {
                        with_guests(run_case);
                    } else {
                        with_game(run_case);
                    }
                }
            }
        },
    );
}

/// `DedicatedServer.initServer` binds the TCP port before `ServerAboutToStartEvent`
/// and world preparation; a bind failure emits only `server_stopped` and leaves
/// the world directory untouched.
#[test]
#[ignore = "daily"]
fn dedicated_port_bind_failure_emits_only_stopped_without_touching_world() {
    use sunrise_client::server::properties::{ServerProperties, SystemEntropy};
    use sunrise_java_properties::Properties;

    struct Log(Arc<Mutex<Vec<&'static str>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Log {
        fn server_about_to_start(
            &self,
            _: &mut dyn ServerHost,
            _: &mut mod_api::ServerAboutToStartEvent,
        ) {
            self.0.lock().unwrap().push("about-to-start");
        }
        fn level_load(&self, _: &mut dyn ServerHost, _: &mut mod_api::LevelLoadEvent<'_>) {
            self.0.lock().unwrap().push("level-load");
        }
        fn server_starting(&self, _: &mut dyn ServerHost, _: &mut mod_api::ServerStartingEvent) {
            self.0.lock().unwrap().push("starting");
        }
        fn server_started(&self, _: &mut dyn ServerHost, _: &mut mod_api::ServerStartedEvent) {
            self.0.lock().unwrap().push("started");
        }
        fn server_stopping(&self, _: &mut dyn ServerHost, _: &mut mod_api::ServerStoppingEvent) {
            self.0.lock().unwrap().push("stopping");
        }
        fn level_unload(&self, _: &mut dyn ServerHost, _: &mut mod_api::LevelUnloadEvent<'_>) {
            self.0.lock().unwrap().push("level-unload");
        }
        fn server_stopped(&self, _: &mut dyn ServerHost, _: &mut mod_api::ServerStoppedEvent) {
            self.0.lock().unwrap().push("stopped");
        }
    }

    let occupied = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = occupied.local_addr().unwrap().port();
    let universe = tempfile::tempdir().unwrap();
    let world_dir = universe.path().join("untouched_world");
    let mut properties = Properties::default();
    properties.put("online-mode", "false".into());
    properties.put("server-ip", "127.0.0.1".into());
    properties.put("server-port", port.to_string());
    let props = ServerProperties::new(properties, &mut SystemEntropy::default()).unwrap();
    let log = Arc::new(Mutex::new(Vec::new()));
    with_mods(
        vec![ModEntry::new("log", Box::new(Log(log.clone())))].into(),
        || {
            with_game(|game| {
                game.options.execution_role = ExecutionRole::DedicatedServer;
                let setup = crate::game::dedicated::Setup {
                    props,
                    universe: universe.path().to_path_buf(),
                    level_name: "untouched_world".into(),
                    port: Some(port),
                    allow_missing_mods: false,
                };
                game.run_dedicated_with_setup(setup).unwrap();
            });
        },
    );
    assert_eq!(*log.lock().unwrap(), ["stopped"]);
    assert!(
        !world_dir.exists(),
        "failed port bind must not create or write world files"
    );
    drop(occupied);
}

/// Configured datapack dimensions open at startup in `LEVEL_STEM` order even
/// when the saved player is in another dimension, and save/close follow that
/// creation order regardless of which level is currently active.
#[test]
#[ignore = "daily"]
fn datapack_dimensions_open_prepare_save_and_close_in_creation_order() {
    use sunrise_client::chunk::{ExtraLevel, LevelShape, LevelShapes};

    struct Log(Arc<Mutex<Vec<String>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Log {
        fn level_load(&self, _: &mut dyn ServerHost, e: &mut mod_api::LevelLoadEvent<'_>) {
            self.0.lock().unwrap().push(format!("load:{}", e.dimension));
        }
        fn level_save(&self, _: &mut dyn ServerHost, e: &mut mod_api::LevelSaveEvent<'_>) {
            self.0.lock().unwrap().push(format!("save:{}", e.dimension));
        }
        fn level_unload(&self, _: &mut dyn ServerHost, e: &mut mod_api::LevelUnloadEvent<'_>) {
            self.0
                .lock()
                .unwrap()
                .push(format!("unload:{}", e.dimension));
        }
    }

    let log = Arc::new(Mutex::new(Vec::new()));
    with_mods(
        vec![ModEntry::new("log", Box::new(Log(log.clone())))].into(),
        || {
            with_game(|game| {
                game.join_world_rest().unwrap();
                let root = tempfile::tempdir().unwrap();
                game.world_root = root.path().to_path_buf();
                game.save = Arc::new(sunrise_client::save::SaveDir::new(
                    game.world_root.clone(),
                    LevelShape::OVERWORLD,
                    Some(Default::default()),
                ));
                let extra: &'static [ExtraLevel] = Box::leak(Box::new([ExtraLevel {
                    id: "test:datapack",
                    dimension_type: "minecraft:overworld",
                    shape: LevelShape::OVERWORLD,
                }]));
                let prev_shapes = game.level_shapes;
                game.level_shapes = LevelShapes {
                    extra,
                    ..prev_shapes
                };
                Dim::set_extra(["test:datapack"].into_iter());
                // Start with the Nether active to verify startup, save, and close
                // all follow `LEVEL_STEM` / creation order rather than active-first.
                game.dim = Dim::Nether;
                game.native_lifecycle_open_started();
                game.open_configured_levels().unwrap();
                assert_eq!(game.dim, Dim::Nether);
                game.prepare_levels().unwrap();
                game.save_game().unwrap();
                assert_eq!(game.dim, Dim::Nether);
                game.close_loaded_levels_for_stop();
                game.native_lifecycle_world_closed();
                game.level_shapes = prev_shapes;
                Dim::set_extra(std::iter::empty());
            });
        },
    );
    assert_eq!(
        *log.lock().unwrap(),
        [
            "load:minecraft:overworld",
            "load:minecraft:the_nether",
            "load:minecraft:the_end",
            "load:test:datapack",
            "save:minecraft:overworld",
            "save:minecraft:the_nether",
            "save:minecraft:the_end",
            "save:test:datapack",
            "unload:minecraft:overworld",
            "unload:minecraft:the_nether",
            "unload:minecraft:the_end",
            "unload:test:datapack",
        ]
    );
}

/// In `MinecraftServer.stopServer`, a per-level close (`save.flush()`) error
/// logs and skips `level_unload` for that level while still closing and firing
/// `level_unload` for the other loaded levels.
#[test]
#[ignore = "daily"]
fn per_level_close_failure_skips_only_that_levels_unload() {
    struct Log(Arc<Mutex<Vec<String>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Log {
        fn level_unload(&self, _: &mut dyn ServerHost, e: &mut mod_api::LevelUnloadEvent<'_>) {
            self.0.lock().unwrap().push(e.dimension.to_owned());
        }
    }

    let log = Arc::new(Mutex::new(Vec::new()));
    with_mods(
        vec![ModEntry::new("log", Box::new(Log(log.clone())))].into(),
        || {
            with_game(|game| {
                game.join_world_rest().unwrap();
                let root = tempfile::tempdir().unwrap();
                game.world_root = root.path().to_path_buf();
                game.save = Arc::new(sunrise_client::save::SaveDir::new(
                    game.world_root.clone(),
                    sunrise_client::save::LevelShape::OVERWORLD,
                    Some(Default::default()),
                ));
                game.native_lifecycle_open_started();
                game.open_configured_levels().unwrap();
                // Point the parked Nether's SaveDir at a directory whose `region`
                // path is a regular file and queue an async chunk write so its
                // close (`save.flush()`) fails with an IO error.
                let broken = root.path().join("broken_nether");
                std::fs::create_dir_all(&broken).unwrap();
                std::fs::write(broken.join("region"), b"not a directory").unwrap();
                let bad_save = Arc::new(sunrise_client::save::SaveDir::new(
                    broken,
                    sunrise_client::save::LevelShape::NETHER,
                    Some(Default::default()),
                ));
                let chunk = Arc::new(sunrise_client::chunk::Chunk::empty(0, 0));
                bad_save
                    .save_later(&[chunk], game.pack.assets.registry, world_biomes!(game))
                    .unwrap();
                game.levels.parked.get_mut(&Dim::Nether).unwrap().save = bad_save;

                game.close_loaded_levels_for_stop();
                game.native_lifecycle_world_closed();
            });
        },
    );
    assert_eq!(
        *log.lock().unwrap(),
        ["minecraft:overworld", "minecraft:the_end"]
    );
}
