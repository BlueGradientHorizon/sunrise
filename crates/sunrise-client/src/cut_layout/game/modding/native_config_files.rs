

#![cfg(test)]
use std::fs;
#[cfg(test)]
pub(crate) mod tests {

use sunrise_gameplay_common::game::native_configs::NativeConfigsGameExt1;
use sunrise_gameplay_common::game::server_side::native_config_files::tests::path;

use super::*;
use mod_api::ConfigFileError;

/// The service through a real game and the hooks' own dispatch: a mod
    /// saves its registered config from `config_loading` and keeps a file in
    /// its folder, the client frame poll hands the saved text back on the client
    /// host, and a call for another mod's file is refused. A fresh process
    /// gives the test its own working folder and `config/`.
    #[test]
    #[ignore = "daily"]
    fn a_mod_saves_its_config_through_the_game_and_the_reload_hears_it() {
        use mod_api::{
            self, ConfigLoadingEvent, ConfigReloadingEvent, Hooks, HostSide, ModEntry, Mods,
            RegisterConfigsEvent,
        };
        use std::sync::Mutex;
        static HEARD: Mutex<Vec<String>> = Mutex::new(Vec::new());
        fn heard() -> std::sync::MutexGuard<'static, Vec<String>> {
            HEARD
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }
        if super::super::native_entity_refs::tests::run_fresh_cases(
            "game::server_side::native_config_files::tests::a_mod_saves_its_config_through_the_game_and_the_reload_hears_it",
            "SUNRISE_CONFIG_FILES_CHILD",
            &["game"],
        ) {
            return;
        }
        struct Proof;
        #[mod_api::__subscriptions]
        impl Hooks for Proof {
            fn register_configs(
                &self,
                _: &dyn mod_api::ConfigContext,
                event: &mut RegisterConfigsEvent<'_>,
            ) {
                event.configs.push(mod_api::Config::new(
                    "proof",
                    mod_api::ConfigKind::Common,
                    "count = 0\n",
                ));
            }
            fn config_loading(
                &self,
                context: &dyn mod_api::ConfigContext,
                event: &mut ConfigLoadingEvent<'_>,
            ) {
                let files = context.config_files();
                heard().push(format!("loading {}", event.config.text.trim()));
                files
                    .write(&path("proof-common.toml"), b"count = 1\n")
                    .unwrap();
                files.write(&path("proof/state.bin"), &[1, 2, 3]).unwrap();
                assert_eq!(files.read(&path("proof/state.bin")).unwrap(), [1, 2, 3]);
                let other = path("other/secret.json");
                assert_eq!(files.read(&other), Err(ConfigFileError::NotOwned));
                assert_eq!(files.write(&other, b"x"), Err(ConfigFileError::NotOwned));
            }
            fn config_reloading(&self, host: HostSide<'_>, event: &mut ConfigReloadingEvent<'_>) {
                let HostSide::Client(context) = host else {
                    panic!("expected the client polling host");
                };
                assert!(
                    context
                        .config_files()
                        .read(&path("proof/state.bin"))
                        .is_ok()
                );
                heard().push(format!("reloading {}", event.config.text.trim()));
            }
        }
        let mods: Mods = vec![ModEntry::new("proof", Box::new(Proof))].into();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_client_game(|game| {
                game.native_configs_client_start();
                game.native_configs_tick_client();
            });
        });
        assert_eq!(*heard(), ["loading count = 0", "reloading count = 1"]);
        let config = std::env::current_dir().unwrap().join("config");
        assert_eq!(fs::read(config.join("proof/state.bin")).unwrap(), [1, 2, 3]);
        assert!(!config.join("other").exists());
        // Outside the mod's hooks the service is no one's.
        assert_eq!(
            sunrise_client_gameplay::game::ClientGame::config_files().read(&path("proof-common.toml")),
            Err(ConfigFileError::NoCallingMod)
        );
    }
}
