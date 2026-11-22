use super::*;
use sunrise_client::crash::{ModCall, enter_native_mod};

struct Root(FileRoot);
impl Root {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "sunrise-config-files-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self(FileRoot {
            path: root,
            bulk: false,
        })
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0.path);
    }
}

fn path(path: &str) -> ConfigPath {
    ConfigPath::new(path).unwrap()
}

fn as_mod<R>(id: &'static str, f: impl FnOnce() -> R) -> R {
    mod_platform::__private::initialize().unwrap();
    let _call = enter_native_mod(ModCall {
        id,
        hook: "world_load",
        part: None,
    });
    f()
}

/// The service through a real game and the owner-bound config listener: a mod
/// saves its registered config from `ConfigEvent::Loading` and keeps a file in
/// its folder, the reload poll hands the saved text back, and the same
/// call for another mod's file is refused. In a fresh process, run in a
/// scratch folder, since `config/` is beside the game's working folder.
#[test]
#[ignore = "daily"]
fn a_mod_saves_its_config_through_the_game_and_the_reload_hears_it() {
    use mod_api::{
        ConfigEvent, ConfigKind, ConfigListener, ConfigSnapshot, ConfigSpec, Hooks, HostSide,
        ModEntry, Mods, Registrar, RegistrationContext,
    };
    use std::sync::{Arc, Mutex};
    static HEARD: Mutex<Vec<String>> = Mutex::new(Vec::new());
    fn heard() -> std::sync::MutexGuard<'static, Vec<String>> {
        HEARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    if super::super::native_entity_refs::tests::run_fresh_cases(
        "game::native_config_files::tests::a_mod_saves_its_config_through_the_game_and_the_reload_hears_it",
        "SUNRISE_CONFIG_FILES_CHILD",
        &["game"],
    ) {
        return;
    }
    fn files(host: &HostSide) -> &'static dyn ConfigFiles {
        match host {
            HostSide::Server(host) => host.config_files(),
            HostSide::Client(host) => host.config_files(),
        }
    }
    struct Proof;
    #[mod_api::__subscriptions]
    impl Hooks for Proof {
        fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
            let listener: Arc<ConfigListener> = Arc::new(
                |host: HostSide<'_>, event: ConfigEvent, config: &ConfigSnapshot| {
                    match event {
                        ConfigEvent::Loading => {
                            let files = files(&host);
                            heard().push(format!("loading {}", config.text.trim()));
                            files
                                .write(&path("proof-common.toml"), b"count = 1\n")
                                .unwrap();
                            files.write(&path("proof/state.bin"), &[1, 2, 3]).unwrap();
                            assert_eq!(files.read(&path("proof/state.bin")).unwrap(), [1, 2, 3]);
                            let other = path("other/secret.json");
                            assert_eq!(files.read(&other), Err(ConfigFileError::NotOwned));
                            assert_eq!(files.write(&other, b"x"), Err(ConfigFileError::NotOwned));
                        }
                        ConfigEvent::Reloading => {
                            heard().push(format!("reloading {}", config.text.trim()));
                        }
                        _ => {}
                    }
                    Ok(())
                },
            );
            r.register_config(
                ConfigSpec::new(ConfigKind::Common, "count = 0\n"),
                Some(listener),
            )
            .unwrap();
        }
    }
    let mods: Mods = vec![ModEntry::new("proof", Box::new(Proof))].into();
    mod_platform::testing::with_mods(mods.clone(), || {
        mod_platform::__private::part_tests::install_parts_fixture(&mods).unwrap();
        super::super::native_entity_refs::tests::with_game(|game| {
            game.native_configs_client_start();
            game.native_configs_poll();
        });
    });
    assert_eq!(*heard(), ["loading count = 0", "reloading count = 1"]);
    let config = std::env::current_dir().unwrap().join("config");
    assert_eq!(fs::read(config.join("proof/state.bin")).unwrap(), [1, 2, 3]);
    assert!(!config.join("other").exists());
    // Outside the mod's hooks the service is no one's.
    assert_eq!(
        Game::config_files().read(&path("proof-common.toml")),
        Err(ConfigFileError::NoCallingMod)
    );
}

#[test]
#[ignore = "daily"]
fn world_bulk_files_survive_save_reload_backup_and_world_deletion() {
    use crate::game::native_entity_refs::tests::{run_fresh_cases, with_game};
    use crate::game::world_loading::WorldRequest;
    if run_fresh_cases(
        "game::native_config_files::tests::world_bulk_files_survive_save_reload_backup_and_world_deletion",
        "SUNRISE_BULK_FILES_CHILD",
        &["game"],
    ) {
        return;
    }
    static WARNINGS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    struct SettingsLog;
    impl log::Log for SettingsLog {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }
        fn log(&self, record: &log::Record<'_>) {
            if record.level() == log::Level::Warn
                && record.target() == "sunrise_world::native_load_context"
            {
                WARNINGS.fetch_add(1, Ordering::Relaxed);
            }
        }
        fn flush(&self) {}
    }
    log::set_logger(&SettingsLog).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    let scratch = Root::new("world-lifecycle");
    let saves = scratch.0.path.join("saves");
    let world = saves.join("Bulk World");
    let request = |create: bool| {
        let rules = std::sync::Arc::new(sunrise_client::gamerules::RuleRegistry::vanilla());
        let options = sunrise_client::title::WorldOptions {
            world_type: sunrise_client::save::WorldPreset::Flat,
            rules: sunrise_client::gamerules::GameRules::with_registry(rules.clone(), &[]),
            ..Default::default()
        };
        let mut request = WorldRequest::new(
            "Bulk World",
            Some("Bulk World"),
            Some(1),
            true,
            create.then_some(&options),
            rules,
        )
        .unwrap();
        request.save_root = world.clone();
        request
    };
    with_game(|game| {
        game.world_root.clear();
        as_mod("fd", || {
            assert_eq!(
                Game::world_data_files(game).read(&path("tiles/nether/0.bin")),
                Err(ConfigFileError::NoWorld)
            );
        });
        game.load_world(request(true)).unwrap();
        as_mod("fd", || {
            let host: &dyn mod_api::ServerHost = game;
            host.data_files()
                .write(&path("server-cache"), b"game-cache")
                .unwrap();
            host.world_data_files()
                .write(&path("tiles/nether/0.bin"), b"world:42")
                .unwrap();
            assert_eq!(
                host.world_data_files()
                    .read(&path("tiles/nether/0.bin"))
                    .unwrap(),
                b"world:42"
            );
            let settings = Game::mod_data_root();
            fs::create_dir_all(settings.join("fd")).unwrap();
            fs::write(settings.join("fd/enabled"), b"true").unwrap();
            WARNINGS.store(0, Ordering::Relaxed);
            let captured = sunrise_world::native_load_context::capture_settings(&settings, ["fd"]);
            assert_eq!(captured["fd"].len(), 1);
            assert_eq!(captured["fd"]["enabled"], "true");
            assert_eq!(WARNINGS.load(Ordering::Relaxed), 0);
        });
        game.save_game().unwrap();
    });
    with_game(|game| {
        game.load_world(request(false)).unwrap();
        as_mod("fd", || {
            let host: &dyn mod_api::ServerHost = game;
            assert_eq!(
                host.world_data_files()
                    .read(&path("tiles/nether/0.bin"))
                    .unwrap(),
                b"world:42"
            );
            assert_eq!(
                host.world_data_files()
                    .list(&path("tiles/nether"), None, 10)
                    .unwrap()[0]
                    .name,
                "0.bin"
            );
        });
    });
    let (backup, _) =
        sunrise_client::title::world_management::backup(&saves, "Bulk World").unwrap();
    let mut zip = zip::ZipArchive::new(fs::File::open(backup).unwrap()).unwrap();
    let mut bytes = Vec::new();
    zip.by_name("Bulk World/sunrise-mod-data/fd/tiles/nether/0.bin")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes, b"world:42");
    sunrise_client::title::world_management::delete(&saves, "Bulk World").unwrap();
    assert!(!world.exists());
    as_mod("fd", || {
        assert_eq!(
            Game::data_files().read(&path("server-cache")).unwrap(),
            b"game-cache"
        );
    });
}
