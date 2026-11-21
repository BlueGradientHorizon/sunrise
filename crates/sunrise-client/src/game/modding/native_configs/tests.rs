use super::*;
use mod_api::ConfigKind::{Client, Common, Server};

#[test]
fn engine_test_host_operation_reads_the_real_phase_provider() {
    use mod_api::{
        __private::{ModPhase, PhaseOwner},
        Host,
        prelude::EntitiesHost,
    };
    Game::initialize_mod_runtime_for_tests();
    struct EmptyHost;
    impl Host for EmptyHost {}
    mod_api::impl_host_areas!(EmptyHost);
    let _phase = PhaseOwner::scope(ModPhase::Setup);
    assert_eq!(mod_api::__private::current_phase(), ModPhase::Setup);
    let refused = std::panic::catch_unwind(|| EmptyHost.entity(1)).unwrap_err();
    let message = refused.downcast_ref::<String>().unwrap();
    assert!(message.contains("Setup"), "{message}");
}

#[test]
#[ignore = "daily"]
fn config_permissions_and_once_per_side_survive_two_worlds() {
    use mod_api::{Hooks, Host, HostSide, HostTransaction, ModEntry};
    use std::sync::{Arc, Mutex};
    struct Probe(Arc<Mutex<Vec<(bool, mod_api::ConfigKind)>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Probe {
        fn register_configs(&self, _: HostSide<'_>, event: &mut mod_api::RegisterConfigsEvent<'_>) {
            for kind in [Client, Common, Server] {
                event
                    .configs
                    .push(mod_api::Config::new("phase_probe", kind, "enabled=true"));
            }
        }
        fn config_loading(
            &self,
            mut host: HostSide<'_>,
            event: &mut mod_api::ConfigLoadingEvent<'_>,
        ) {
            assert_eq!(
                mod_api::__private::current_phase(),
                mod_api::__private::ModPhase::Config
            );
            let read: &dyn Host = match &host {
                HostSide::Server(host) => &**host,
                HostSide::Client(host) => &**host,
            };
            let _ = read.setting("phase_probe", "unused");
            let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read.entity(1)))
                .unwrap_err();
            assert_eq!(
                refused
                    .downcast_ref::<mod_api::__private::PhaseRefusal>()
                    .map(|refusal| refusal.phase),
                Some(mod_api::__private::ModPhase::Config)
            );
            if let HostSide::Server(host) = &mut host {
                let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _: Result<(), ()> = host.transaction(|_, _| Ok(()));
                }))
                .unwrap_err();
                assert_eq!(
                    refused
                        .downcast_ref::<mod_api::__private::PhaseRefusal>()
                        .map(|refusal| refusal.phase),
                    Some(mod_api::__private::ModPhase::Config)
                );
            }
            self.0.lock().unwrap().push((true, event.config.kind));
        }
        fn config_unloading(
            &self,
            _: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::ConfigUnloadingEvent<'_>,
        ) {
            assert_eq!(
                mod_api::__private::current_phase(),
                mod_api::__private::ModPhase::World
            );
            self.0.lock().unwrap().push((false, event.config.kind));
        }
    }
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mods = vec![ModEntry::new("phase_probe", Box::new(Probe(calls.clone())))].into();
    mod_platform::testing::with_mods(mods, || {
        crate::game::native_entity_refs::tests::with_client_game(|game| {
            game.mod_phase =
                mod_api::__private::PhaseOwner::new(mod_api::__private::ModPhase::Setup);
            game.native_configs_client_start();
            game.native_configs_client_start();
            game.options.execution_role =
                crate::game::execution_role::ExecutionRole::IntegratedServer;
            for world in ["phase-world-one", "phase-world-two"] {
                game.native_configs_server_start(Path::new(world), false);
                game.native_configs_server_stop();
            }
        });
    });
    assert_eq!(
        *calls.lock().unwrap(),
        [
            (true, Client),
            (true, Common),
            (true, Server),
            (false, Server),
            (true, Server),
            (false, Server),
        ]
    );
}

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "sunrise-native-configs-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_missing_file_gets_its_default_and_an_edited_one_is_read() {
    let root = Folder::new("default");
    let config = root.0.join("config");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("gt-common.toml"), "voltage = 512\n").unwrap();
    let mut configs = NativeConfigs::default();
    let loaded = configs.load(
        vec![
            mod_api::Config::new("gt", Client, "hud = true\n"),
            mod_api::Config::new("gt", Common, "voltage = 32\n"),
            mod_api::Config::new("gt", Server, "ores = 1\n"),
        ],
        &[Client, Common],
        &config,
        None,
    );
    let got: Vec<_> = loaded
        .iter()
        .map(|f| (f.file.as_str(), f.text.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("gt-client.toml", "hud = true\n"),
            ("gt-common.toml", "voltage = 512\n")
        ]
    );
    assert_eq!(
        fs::read_to_string(config.join("gt-client.toml")).unwrap(),
        "hud = true\n"
    );
    // Another side's kind is neither read nor written.
    assert!(!config.join("gt-server.toml").exists());
}

#[test]
fn a_world_overrides_a_server_config_and_the_folder_serves_the_rest() {
    let root = Folder::new("world");
    let (config, world) = (root.0.join("config"), root.0.join("saves/W"));
    fs::create_dir_all(world.join("serverconfig/gt")).unwrap();
    fs::write(world.join("serverconfig/gt/ores.toml"), "veins = 9\n").unwrap();
    let mut configs = NativeConfigs::default();
    let loaded = configs.load(
        vec![
            mod_api::Config::new("gt", Server, "veins = 3\n").file("gt/ores.toml"),
            mod_api::Config::new("gt", Server, "tiers = 4\n"),
            // A Common config of the world's folder is not looked for.
            mod_api::Config::new("gt", Common, "a = 1\n"),
        ],
        &[Server],
        &config,
        Some(&world),
    );
    let got: Vec<_> = configs
        .files
        .iter()
        .map(|t| (t.path.clone(), t.config.text.as_str()))
        .collect();
    assert_eq!(loaded.len(), 2);
    assert_eq!(
        got,
        [
            (world.join("serverconfig/gt/ores.toml"), "veins = 9\n"),
            (config.join("gt-server.toml"), "tiers = 4\n"),
        ]
    );
    // The world's copy stands for the default: nothing is written beside it.
    assert!(!config.join("gt/ores.toml").exists());
    assert_eq!(configs.drop_server().len(), 2);
    assert!(configs.files.is_empty());
}

#[test]
fn a_file_not_the_mods_own_or_registered_twice_is_skipped() {
    let root = Folder::new("skip");
    let config = root.0.join("config");
    let mut configs = NativeConfigs::default();
    let loaded = configs.load(
        vec![
            mod_api::Config::new("a", Common, "x").file("../options.txt"),
            mod_api::Config::new("a", Common, "x").file(""),
            mod_api::Config::new("a", Common, "first"),
            mod_api::Config::new("a", Common, "again"),
            // Another mod's file, as `Host::config_files` refuses it.
            mod_api::Config::new("b", Common, "second").file("a-common.toml"),
        ],
        &[Common],
        &config,
        None,
    );
    assert_eq!(
        loaded.iter().map(|f| f.mod_id.as_str()).collect::<Vec<_>>(),
        ["a"]
    );
    assert!(!root.0.join("options.txt").exists());
}

#[test]
fn a_change_reloads_once_and_a_touch_without_one_does_not() {
    let root = Folder::new("reload");
    let config = root.0.join("config");
    let mut configs = NativeConfigs::default();
    configs.load(
        vec![mod_api::Config::new("gt", Common, "a = 1\n")],
        &[Common],
        &config,
        None,
    );
    let path = config.join("gt-common.toml");
    assert!(configs.changed().is_empty());
    fs::write(&path, "a = 2\n").unwrap();
    let changed = configs.changed();
    assert_eq!(
        changed.iter().map(|f| f.text.as_str()).collect::<Vec<_>>(),
        ["a = 2\n"]
    );
    assert!(configs.changed().is_empty());
    // Saved again with the same text: nothing to reload.
    fs::write(&path, "a = 2\n").unwrap();
    assert!(configs.changed().is_empty());
    // Same length, written at once after the load, as a mod's own save
    // from `config_loading` is: still a change.
    fs::write(&path, "a = 3\n").unwrap();
    assert_eq!(configs.changed().len(), 1);
    fs::write(&path, "a = 2\n").unwrap();
    configs.changed();
    // Gone: the last text stays, and nothing fires.
    fs::remove_file(&path).unwrap();
    assert!(configs.changed().is_empty());
    assert_eq!(configs.files[0].config.text, "a = 2\n");
}

#[test]
fn files_are_looked_at_once_a_second_and_never_without_configs() {
    let mut configs = NativeConfigs::default();
    let start = Instant::now();
    assert!(!configs.due(start));
    configs.files.push(Tracked {
        config: ConfigFile::from(mod_api::__private::built::ConfigFile {
            mod_id: "gt".into(),
            kind: Common,
            file: "gt-common.toml".into(),
            text: String::new(),
        }),
        path: PathBuf::new(),
    });
    assert!(configs.due(start));
    assert!(!configs.due(start + Duration::from_millis(999)));
    assert!(configs.due(start + CHECK_EVERY));
}
