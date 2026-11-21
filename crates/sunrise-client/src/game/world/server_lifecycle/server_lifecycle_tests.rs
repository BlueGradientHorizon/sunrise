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
                    "opening-failed-init",
                    "bind-failed-init",
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
                        let adopted =
                            !matches!(case, "unopened" | "bind-failed-init" | "locked-unadopted");
                        if adopted || case == "locked-unadopted" {
                            game.session_lock =
                                Some(sunrise_client::save::SessionLock::acquire(&root).unwrap());
                        }
                        // The lock is held while tags are readable, before the prepared
                        // level replaces the placeholder. Only adoption may save.
                        game.world_adopted = adopted;
                        if matches!(case, "unopened" | "bind-failed-init" | "locked-unadopted") {
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
                            if case == "bind-failed-init" {
                                return Ok(super::ServerRunOutcome::FailedInit(
                                    std::io::Error::other("fixture initialization error"),
                                ));
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
                            if case == "opening-failed-init" {
                                return Ok(super::ServerRunOutcome::FailedInit(
                                    std::io::Error::other("fixture initialization error"),
                                ));
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
                            matches!(case, "normal" | "runtime-extra-level"),
                            "{dedicated} {case}"
                        );
                        if config_panic || matches!(case, "stopping-panic" | "save-panic") {
                            let message = result.unwrap_err().to_string();
                            let needle = match case {
                                "stopping-panic" => "stopping listener",
                                "save-panic" | "config-unloading-save-panic" => "save listener",
                                "config-unloading-logout-panic" => "logout listener",
                                _ => "config unloading listener",
                            };
                            assert!(message.contains(needle), "{dedicated} {case}: {message}");
                        }
                        let expected: &[&str] = match case {
                            "bind-failed-init" => &["stopped"],
                            "unopened" => &["about-to-start", "stopped", "config-unloading"],
                            "locked-unadopted" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "level-unload:minecraft:overworld",
                                "stopped",
                                "config-unloading",
                            ],
                            "startup-error" | "opening-failed-init" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "level-save:minecraft:overworld",
                                "saved",
                                "level-unload:minecraft:overworld",
                                "stopped",
                                "config-unloading",
                            ],
                            "tick-error" | "panic" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "level-save:minecraft:overworld",
                                "saved",
                                "level-unload:minecraft:overworld",
                                "stopped",
                                "config-unloading",
                            ],
                            "save-error" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "stopping",
                                "level-save:minecraft:overworld",
                                "stopped",
                                "config-unloading",
                            ],
                            "save-panic" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "stopping",
                                "level-save:minecraft:overworld",
                                "saved",
                                "stopped",
                                "config-unloading",
                            ],
                            "runtime-extra-level" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "level-load:test:pocket",
                                "stopping",
                                "level-save:minecraft:overworld",
                                "level-save:test:pocket",
                                "saved",
                                "level-unload:minecraft:overworld",
                                "level-unload:test:pocket",
                                "stopped",
                                "config-unloading",
                            ],
                            "config-unloading-logout-panic" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "stopping",
                                "logout",
                                "level-save:minecraft:overworld",
                                "saved",
                                "level-unload:minecraft:overworld",
                                "stopped",
                                "config-unloading",
                            ],
                            "config-unloading-save-panic" => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "stopping",
                                "level-save:minecraft:overworld",
                                "saved",
                                "stopped",
                                "config-unloading",
                            ],
                            _ => &[
                                "about-to-start",
                                "level-load:minecraft:overworld",
                                "starting",
                                "started",
                                "stopping",
                                "level-save:minecraft:overworld",
                                "saved",
                                "level-unload:minecraft:overworld",
                                "stopped",
                                "config-unloading",
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
                        if matches!(case, "unopened" | "bind-failed-init" | "locked-unadopted") {
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
                let error = game.run_dedicated_with_setup(setup).unwrap_err();
                assert!(
                    error
                        .to_string()
                        .starts_with("Failed to initialize server:")
                );
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

/// The real world-open path promotes the Overworld, initializes its spawn, then
/// walks the resolved stems. Later saves and closes reuse that creation order.
#[test]
#[ignore = "daily"]
fn datapack_dimensions_open_prepare_save_and_close_in_creation_order() {
    use crate::game::world_loading::WorldRequest;
    use std::fs;
    use sunrise_client::save::Level;

    struct Log(Arc<Mutex<Vec<String>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Log {
        fn level_load(&self, _: &mut dyn ServerHost, e: &mut mod_api::LevelLoadEvent<'_>) {
            self.0.lock().unwrap().push(format!("load:{}", e.dimension));
        }
        fn create_spawn_position(
            &self,
            _: &mut dyn ServerHost,
            e: &mut mod_api::CreateSpawnPositionEvent<'_>,
        ) -> mod_api::Flow {
            self.0.lock().unwrap().push("initialize-spawn".into());
            *e.position = [32, 73, 16];
            mod_api::Flow::Cancel
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

    let root = tempfile::tempdir().unwrap();
    let pack = root.path().join("datapacks/stems");
    let dimensions = pack.join("data/test/dimension");
    fs::create_dir_all(&dimensions).unwrap();
    fs::write(
        pack.join("pack.mcmeta"),
        r#"{"pack":{"description":"Lifecycle fixture","min_format":[121,0],"max_format":[121,0]}}"#,
    )
    .unwrap();
    fs::write(
        root.path().join("datapacks.json"),
        r#"{"enabled":["vanilla","file/stems"],"disabled":[]}"#,
    )
    .unwrap();
    for name in ["last", "first"] {
        fs::write(
            dimensions.join(format!("{name}.json")),
            r#"{"type":"minecraft:overworld","generator":{"type":"minecraft:flat","settings":{
                "biome":"minecraft:plains","features":false,"lakes":false,"structure_overrides":[],
                "layers":[{"height":1,"block":"minecraft:bedrock"}]}}}"#,
        )
        .unwrap();
    }
    let log = Arc::new(Mutex::new(Vec::new()));
    with_mods(
        vec![ModEntry::new("log", Box::new(Log(log.clone())))].into(),
        || {
            with_game(|game| {
                game.options.execution_role = ExecutionRole::DedicatedServer;
                game.join_world_rest().unwrap();
                let mut request = WorldRequest::new(
                    "stems",
                    Some("stems"),
                    Some(1),
                    false,
                    None,
                    Level::default().rule_registry,
                )
                .unwrap();
                request.save_root = root.path().to_path_buf();
                let prepared = game.prepare_world_and_rules(request).unwrap();
                assert!(prepared.level.world_spawn.is_none());
                let order: Vec<_> = std::iter::once(Dim::Overworld)
                    .chain(
                        prepared
                            .configured_levels
                            .iter()
                            .copied()
                            .filter(|d| *d != Dim::Overworld),
                    )
                    .map(Dim::id)
                    .collect();
                let mut ids = order.clone();
                ids.sort_unstable();
                assert_eq!(
                    ids,
                    [
                        "minecraft:overworld",
                        "minecraft:the_end",
                        "minecraft:the_nether",
                        "test:first",
                        "test:last"
                    ]
                );
                game.native_lifecycle_open_started();
                game.finish_open_world(prepared).unwrap();
                game.prepare_levels().unwrap();
                assert_eq!(game.level.world_spawn, Some([32, 73, 16]));
                // Switching the active level must not put it first in saves or close.
                game.enter_level(Dim::Nether).unwrap();
                game.save_game().unwrap();
                assert_eq!(game.dim, Dim::Nether);
                game.close_loaded_levels_for_stop();
                game.native_lifecycle_world_closed();
                Dim::set_extra(std::iter::empty());

                let mut expected =
                    vec!["load:minecraft:overworld".into(), "initialize-spawn".into()];
                expected.extend(order[1..].iter().map(|id| format!("load:{id}")));
                expected.extend(order.iter().map(|id| format!("save:{id}")));
                expected.extend(order.iter().map(|id| format!("unload:{id}")));
                assert_eq!(*log.lock().unwrap(), expected);
            })
        },
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
                game.level.world_spawn = Some([0, 64, 0]);
                game.native_lifecycle_open_started();
                game.open_configured_levels(&[Dim::Overworld, Dim::Nether, Dim::End])
                    .unwrap();
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

/// Final level.dat writes must not serialize the integrated server's shell as its owner.
#[test]
#[ignore = "daily"]
fn integrated_shutdown_reopens_the_authoritative_owner_or_keeps_an_interrupted_save() {
    use crate::game::lan::Owner;
    use sunrise_client::{
        items::ItemStack,
        lan::playerdata,
        save::{self, Level, LevelShape, SaveDir},
    };
    use sunrise_core::nbt::Tag;

    for (interrupted, legacy) in [(false, false), (true, false), (true, true)] {
        with_guests(|game| {
            game.join_world_rest().unwrap();
            let root = tempfile::tempdir().unwrap();
            let dir = root.path();
            let items = game.res.items();
            let mut old = Level {
                player_uuid: Some(playerdata::uuid_ints(12)),
                name: "previous".into(),
                position: Some([1.0, 72.0, 3.0]),
                dimension: "minecraft:overworld".into(),
                xp_level: 4,
                xp_total: 14,
                inventory: vec![(0, "minecraft:stone".into(), 2, 0)],
                ..Level::default()
            };
            let old_player = old.player_tag(items);
            let level_dat = dir.join("level.dat");
            if legacy {
                old.write_without_player(dir).unwrap();
                let mut root = save::read_dat(&level_dat).unwrap();
                let Some(Tag::Compound(data)) = root.get_mut("Data") else {
                    panic!("level.dat has a Data compound");
                };
                data.insert("Player".into(), old_player.clone());
                data.remove(&"singleplayer_uuid".into());
                save::write_dat(&level_dat, root).unwrap();
            } else {
                old.write(dir, items).unwrap();
            }
            let player_file = playerdata::path(dir, 12);
            let prior_bytes = std::fs::read(&player_file).ok();

            let host = game.lan.as_mut().unwrap();
            host.owner = Some(Owner {
                uuid: 12,
                saved: None,
            });
            host.hub.singleplayer_owner = Some(12);
            host.hub
                .update_guest(12, |guest| {
                    guest.dimension = "minecraft:the_nether".into();
                    guest.position = [8.0, 90.0, -4.0];
                    guest.xp_level = 11;
                    guest.xp_total = 90;
                    guest.inventory[0] = Some(ItemStack {
                        item: "minecraft:diamond".into(),
                        count: 9,
                        damage: 0,
                        extra: None,
                    });
                })
                .unwrap();
            game.world_root = dir.to_path_buf();
            game.save = Arc::new(SaveDir::new(
                dir.to_path_buf(),
                LevelShape::OVERWORLD,
                Some(Default::default()),
            ));
            game.session_lock = Some(save::SessionLock::acquire(dir).unwrap());
            game.world_adopted = true;
            game.level = old;
            game.level.name = "stopped".into();
            if interrupted {
                game.interrupted_turns.push(12);
            }
            game.run_server_lifecycle(false, |_| Ok(super::ServerRunOutcome::Orderly))
                .unwrap();

            let saved = Level::read(dir).unwrap();
            assert_eq!(saved.name, "stopped");
            if interrupted {
                assert_eq!(std::fs::read(&player_file).ok(), prior_bytes);
                assert_eq!(saved.dimension, "minecraft:overworld");
                assert_eq!((saved.xp_level, saved.xp_total), (4, 14));
                assert_eq!(saved.inventory, [(0, "minecraft:stone".to_owned(), 2, 0)]);
                if legacy {
                    let root = save::read_dat(&level_dat).unwrap();
                    assert_eq!(
                        root.get("Data").and_then(|data| data.get("Player")),
                        Some(&old_player)
                    );
                }
            } else {
                assert!(saved.singleplayer_file);
                assert_eq!(saved.dimension, "minecraft:the_nether");
                assert_eq!(saved.position, Some([8.0, 90.0, -4.0]));
                assert_eq!((saved.xp_level, saved.xp_total), (11, 90));
                assert_eq!(saved.inventory, [(0, "minecraft:diamond".to_owned(), 9, 0)]);
            }
        });
    }
}
