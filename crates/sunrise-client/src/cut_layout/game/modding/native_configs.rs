#![cfg(test)]
#[cfg(test)]
pub(crate) mod tests {
    use mod_api::ConfigKind::{Client, Common, Server};
    use sunrise_client_gameplay::game::native_hooks::mod_configs::GameHooks;
    use sunrise_gameplay_common::game::native_configs::NativeConfigsGameExt1;
    use sunrise_gameplay_common::game::native_configs::{
            CHECK_EVERY, ConfigFile, Duration, Instant, NativeConfigs, Path, PathBuf, Tracked, fs,
        };

    #[test]
    #[ignore = "daily"]
    fn panicking_config_hooks_keep_live_writes_and_do_not_stop_later_listeners() {
        use mod_api::{
            Hooks, HostSide, ModEntry, Registrar, RegistrationContext, ServerHost, SettingSpec,
        };
        use std::sync::{Arc, Mutex};
        use {
            sunrise_gameplay::game::native_entity_refs::tests::run_fresh_cases,
            sunrise_gameplay::game::native_entity_refs::tests::with_game,
        };

        if run_fresh_cases(
            "game::native_configs::tests::panicking_config_hooks_keep_live_writes_and_do_not_stop_later_listeners",
            "SUNRISE_CONFIG_PANIC_CHILD",
            &["server"],
        ) {
            return;
        }

        struct Broken;
        #[mod_api::__subscriptions]
        impl Hooks for Broken {
            fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
                for (key, title) in [("reload", "fault.reload"), ("unload", "fault.unload")] {
                    registrar
                        .register_setting(SettingSpec::toggle(key, title, false))
                        .unwrap();
                }
            }

            fn config_reloading(
                &self,
                host: HostSide<'_>,
                _: &mut mod_api::ConfigReloadingEvent<'_>,
            ) {
                let HostSide::Server(host) = host else {
                    panic!("expected server polling")
                };
                host.set_setting("fault", "reload", "true");
                panic!("broken reload listener");
            }

            fn config_unloading(
                &self,
                host: &mut dyn ServerHost,
                _: &mut mod_api::ConfigUnloadingEvent<'_>,
            ) {
                host.set_setting("fault", "unload", "true");
                panic!("broken unload listener");
            }
        }

        struct Later(Arc<Mutex<Vec<&'static str>>>);
        #[mod_api::__subscriptions]
        impl Hooks for Later {
            fn config_reloading(&self, _: HostSide<'_>, _: &mut mod_api::ConfigReloadingEvent<'_>) {
                self.0.lock().unwrap().push("reload");
            }

            fn config_unloading(
                &self,
                _: &mut dyn ServerHost,
                _: &mut mod_api::ConfigUnloadingEvent<'_>,
            ) {
                self.0.lock().unwrap().push("unload");
            }
        }

        let observed = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![
            ModEntry::new("fault", Box::new(Broken)),
            ModEntry::new("later", Box::new(Later(observed.clone()))),
        ]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                let config = ConfigFile::from(mod_api::__private::built::ConfigFile {
                    mod_id: "fault".into(),
                    kind: Server,
                    file: "fault-server.toml".into(),
                    text: String::new(),
                });
                game.native_config_reloading(&config);
                game.native_config_unloading(&config);
                assert_eq!(game.setting("fault", "reload").as_deref(), Some("true"));
                assert_eq!(game.setting("fault", "unload").as_deref(), Some("true"));
                // These writes lack a journal. Reset them outside dispatch to make another
                // invocation of either disabled callback observable.
                game.set_setting("fault", "reload", "false");
                game.set_setting("fault", "unload", "false");
                game.native_config_reloading(&config);
                game.native_config_unloading(&config);
                assert_eq!(game.setting("fault", "reload").as_deref(), Some("false"));
                assert_eq!(game.setting("fault", "unload").as_deref(), Some("false"));
                assert_eq!(
                    observed.lock().unwrap().as_slice(),
                    ["reload", "unload", "reload", "unload"]
                );
                assert_eq!(
                    mod_api::__private::take_hook_panics(),
                    [
                        mod_api::__private::HookPanicReport {
                            mod_id: "fault",
                            hook: "config_reloading"
                        },
                        mod_api::__private::HookPanicReport {
                            mod_id: "fault",
                            hook: "config_unloading"
                        },
                    ]
                );
            })
        });
    }

    #[test]
    #[ignore = "daily"]
    fn config_permissions_and_once_per_side_survive_two_worlds() {
        use mod_api::{
            ConfigContext, Hooks, ModEntry, Registrar, RegistrationContext, SettingSpec,
        };
        use std::sync::{Arc, Mutex};
        use {
            crate::game::server_side::native_entity_refs::tests::with_client_game,
            sunrise_gameplay::game::native_entity_refs::tests::run_fresh_cases,
            sunrise_gameplay::game::native_entity_refs::tests::with_game,
        };

        if run_fresh_cases(
            "game::native_configs::tests::config_permissions_and_once_per_side_survive_two_worlds",
            "SUNRISE_CONFIG_LIFECYCLE_CHILD",
            &["lifecycle"],
        ) {
            return;
        }

        struct Probe(Arc<Mutex<Vec<(bool, mod_api::ConfigKind)>>>);
        #[mod_api::__subscriptions]
        impl Hooks for Probe {
            fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
                registrar
                    .register_setting(SettingSpec::toggle(
                        "unloaded",
                        "phase_probe.unloaded",
                        false,
                    ))
                    .unwrap();
            }

            fn register_configs(
                &self,
                context: &dyn ConfigContext,
                event: &mut mod_api::RegisterConfigsEvent<'_>,
            ) {
                let _ = context.setting("phase_probe", "unused");
                for kind in [Client, Common, Server] {
                    event
                        .configs
                        .push(mod_api::Config::new("phase_probe", kind, "enabled=true"));
                }
            }
            fn config_loading(
                &self,
                context: &dyn ConfigContext,
                event: &mut mod_api::ConfigLoadingEvent<'_>,
            ) {
                let _ = context.setting("phase_probe", "unused");
                let _ = context.config_files();
                self.0.lock().unwrap().push((true, event.config.kind));
            }
            fn config_unloading(
                &self,
                context: &mut dyn mod_api::ServerHost,
                event: &mut mod_api::ConfigUnloadingEvent<'_>,
            ) {
                let next = match context.setting("phase_probe", "unloaded").as_deref() {
                    Some("true") => "false",
                    _ => "true",
                };
                context.set_setting("phase_probe", "unloaded", next);
                self.0.lock().unwrap().push((false, event.config.kind));
            }
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![ModEntry::new("phase_probe", Box::new(Probe(calls.clone())))].into();
        mod_platform::testing::with_mods(mods, || {
            with_client_game(|game| {
                game.native_configs_client_start();
                game.native_configs_client_start();
            });
            with_game(|game| {
                for (world, expected) in [("phase-world-one", "true"), ("phase-world-two", "false")]
                {
                    game.native_configs_server_start(Path::new(world), false);
                    game.native_configs_server_stop();
                    assert_eq!(
                        game.setting("phase_probe", "unloaded").as_deref(),
                        Some(expected)
                    );
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
}
