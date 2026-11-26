use super::Game;
use sunrise_gameplay_common::game::native_config_file_services::{FileRoot, list, read, remove, stat, write};
use mod_api::{ConfigEntry, ConfigFileError, ConfigFiles, ConfigPath, ConfigStat};
#[cfg(test)]
use std::fs;

impl<__Side: sunrise_gameplay_common::game::Side> Game<'_, __Side> {
    #[inline(always)]
    pub(in crate::game) fn config_files() -> &'static dyn ConfigFiles {
        sunrise_gameplay_common::game::native_config_file_services::config_files()
    }
    #[inline(always)]
    pub(in crate::game) fn data_files() -> &'static dyn ConfigFiles {
        sunrise_gameplay_common::game::native_config_file_services::data_files()
    }

    #[inline(always)]
    pub(in crate::game) fn is_loaded(id: &str) -> bool {
        sunrise_gameplay_common::game::native_config_file_services::is_loaded(id)
    }
}

impl crate::game::ServerGame<'_> {
    pub(in crate::game) fn world_data_files(&self) -> &dyn ConfigFiles {
        self
    }
}

fn world_root(
    game: &crate::game::ServerGame<'_>,
) -> Result<FileRoot, ConfigFileError> {
    if game.server.world_root.as_os_str().is_empty() {
        return Err(ConfigFileError::NoWorld);
    }
    Ok(FileRoot {
        path: game.server.world_root.clone(),
        bulk: true,
    })
}

impl ConfigFiles for crate::game::ServerGame<'_> {
    fn read(&self, file: &ConfigPath) -> Result<Vec<u8>, ConfigFileError> {
        read(&world_root(self)?, file)
    }
    fn write(&self, file: &ConfigPath, bytes: &[u8]) -> Result<(), ConfigFileError> {
        write(&world_root(self)?, file, bytes)
    }
    fn list(
        &self,
        folder: &ConfigPath,
        after: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ConfigEntry>, ConfigFileError> {
        list(&world_root(self)?, folder, after, limit)
    }
    fn stat(&self, path: &ConfigPath) -> Result<Option<ConfigStat>, ConfigFileError> {
        stat(&world_root(self)?, path)
    }
    fn remove(&self, file: &ConfigPath) -> Result<bool, ConfigFileError> {
        remove(&world_root(self)?, file)
    }
}

#[cfg(test)]
mod tests {
use sunrise_gameplay_common::game::server_side::native_config_files::tests::as_mod;

use sunrise_gameplay_common::game::server_side::native_config_files::tests::path;

    use super::*;
    use mod_api::{CONFIG_FILE_MAX, ConfigEntryKind, DATA_FILE_MAX};
    use std::io::Read;
    use std::sync::atomic::Ordering;
    use sunrise_level::crash::{ModCall, enter_native_mod};

    
    
    

    

    

    #[test]
    fn a_mod_writes_reads_lists_and_stats_its_own_files() {
        let root = Root::new("own");
        let root = &root.0;
        as_mod("fd", || {
            assert_eq!(stat(root, &path("fd/state.json")), Ok(None));
            write(root, &path("fd/state.json"), b"{\"a\":1}").unwrap();
            write(root, &path("fd.json"), b"top").unwrap();
            assert_eq!(read(root, &path("fd/state.json")).unwrap(), b"{\"a\":1}");
            assert_eq!(read(root, &path("fd.json")).unwrap(), b"top");
            // A second write replaces the bytes and leaves no temporary file.
            write(root, &path("fd/state.json"), b"{}").unwrap();
            let entries = list(root, &path("fd"), None, 1024).unwrap();
            assert_eq!(
                entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
                ["state.json"]
            );
            assert_eq!(entries[0].stat.kind, ConfigEntryKind::File);
            assert_eq!(entries[0].stat.len, 2);
            assert!(entries[0].stat.modified_millis > 0);
            assert_eq!(
                stat(root, &path("fd")).unwrap().map(|s| s.kind),
                Some(ConfigEntryKind::Folder)
            );
            assert_eq!(read(root, &path("fd")), Err(ConfigFileError::NotAFile));
            assert_eq!(
                list(root, &path("fd.json"), None, 1024),
                Err(ConfigFileError::NotAFolder)
            );
            assert_eq!(read(root, &path("fd/gone")), Err(ConfigFileError::NotFound));
        });
        assert_eq!(fs::read(root.path.join("fd/state.json")).unwrap(), b"{}");
    }

    #[test]
    fn a_mod_removes_only_its_own_files() {
        let root = Root::new("remove");
        let root = &root.0;
        as_mod("b", || write(root, &path("b.json"), b"b's").unwrap());
        as_mod("fd", || {
            write(root, &path("fd/old.bin"), b"x").unwrap();
            write(root, &path("fd/new.bin"), b"y").unwrap();
            assert_eq!(remove(root, &path("fd/old.bin")), Ok(true));
            let names: Vec<_> = list(root, &path("fd"), None, 1024)
                .unwrap()
                .into_iter()
                .map(|e| e.name)
                .collect();
            assert_eq!(names, ["new.bin"]);
            assert_eq!(remove(root, &path("fd/old.bin")), Ok(false));
            assert_eq!(remove(root, &path("fd/absent/a.bin")), Ok(false));
            assert_eq!(remove(root, &path("fd")), Err(ConfigFileError::NotAFile));
            fs::create_dir(root.path.join("fd/folder")).unwrap();
            assert_eq!(
                remove(root, &path("fd/folder")),
                Err(ConfigFileError::NotAFile)
            );
            assert_eq!(
                remove(root, &path("b.json")),
                Err(ConfigFileError::NotOwned)
            );
        });
        assert!(root.path.join("fd/folder").is_dir());
        assert_eq!(fs::read(root.path.join("b.json")).unwrap(), b"b's");
    }

    #[test]
    fn a_file_named_for_the_mod_cant_block_its_folder_and_list_hides_write_temps() {
        let root = Root::new("folder-name");
        let root = &root.0;
        as_mod("fd", || {
            assert_eq!(
                write(root, &path("fd"), b"x"),
                Err(ConfigFileError::NotAFile)
            );
            assert!(!root.path.join("fd").exists());
            write(root, &path("fd/state.json"), b"{}").unwrap();
            // What a crash between the temp file and the rename leaves behind.
            fs::write(root.path.join("fd.json.1-0.tmp"), b"half").unwrap();
            fs::write(root.path.join("fd/state.json.1-0.tmp"), b"half").unwrap();
            fs::write(root.path.join("fd/notes.tmp"), b"mine").unwrap();
            assert_eq!(read(root, &path("fd.json.1-0.tmp")).unwrap(), b"half");
            // `list` leaves out a write's temporary file, not a mod's own `.tmp`.
            let names: Vec<_> = list(root, &path("fd"), None, 1024)
                .unwrap()
                .into_iter()
                .map(|e| e.name)
                .collect();
            assert_eq!(names, ["notes.tmp", "state.json"]);
        });
    }

    #[test]
    fn a_mod_never_reaches_another_mods_files() {
        let root = Root::new("other");
        let root = &root.0;
        as_mod("b", || write(root, &path("b/secret.json"), b"b's").unwrap());
        as_mod("a", || {
            for other in ["b/secret.json", "b.json", "b-common.toml"] {
                let other = path(other);
                assert_eq!(read(root, &other), Err(ConfigFileError::NotOwned));
                assert_eq!(write(root, &other, b"a's"), Err(ConfigFileError::NotOwned));
                assert_eq!(stat(root, &other), Err(ConfigFileError::NotOwned));
            }
            assert_eq!(
                list(root, &path("b"), None, 1024),
                Err(ConfigFileError::NotOwned)
            );
        });
        assert_eq!(fs::read(root.path.join("b/secret.json")).unwrap(), b"b's");
        // Outside any mod's call nothing is anyone's.
        assert_eq!(
            read(root, &path("b/secret.json")),
            Err(ConfigFileError::NoCallingMod)
        );
    }

    

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_never_leads_out_of_config() {
        let root = Root::new("link");
        let outside = Root::new("link-outside");
        fs::write(outside.0.path.join("passwd"), b"outside").unwrap();
        std::os::unix::fs::symlink(&outside.0.path, root.0.path.join("fd")).unwrap();
        std::os::unix::fs::symlink(outside.0.path.join("passwd"), root.0.path.join("fd.json"))
            .unwrap();
        as_mod("fd", || {
            assert_eq!(
                read(&root.0, &path("fd/passwd")),
                Err(ConfigFileError::Symlink)
            );
            assert_eq!(
                write(&root.0, &path("fd/new"), b"x"),
                Err(ConfigFileError::Symlink)
            );
            assert_eq!(
                read(&root.0, &path("fd.json")),
                Err(ConfigFileError::Symlink)
            );
            assert_eq!(
                list(&root.0, &path("fd"), None, 1024),
                Err(ConfigFileError::Symlink)
            );
            for linked in ["fd/passwd", "fd.json"] {
                assert_eq!(
                    remove(&root.0, &path(linked)),
                    Err(ConfigFileError::Symlink)
                );
            }
        });
        assert!(!outside.0.path.join("new").exists());
        assert!(outside.0.path.join("passwd").exists());
        assert!(root.0.path.join("fd.json").is_symlink());
    }

    #[test]
    fn a_file_holds_four_mebibytes_at_most() {
        let root = Root::new("size");
        let root = &root.0;
        let max = vec![b'x'; CONFIG_FILE_MAX as usize];
        as_mod("fd", || {
            write(root, &path("fd/max"), &max).unwrap();
            assert_eq!(read(root, &path("fd/max")).unwrap().len(), max.len());
            let over = [max.as_slice(), b"x"].concat();
            assert_eq!(
                write(root, &path("fd/over"), &over),
                Err(ConfigFileError::TooLarge)
            );
            fs::write(root.path.join("fd/over"), &over).unwrap();
            assert_eq!(read(root, &path("fd/over")), Err(ConfigFileError::TooLarge));
        });
    }
    #[test]
    fn bulk_files_are_mod_bound_paged_and_separate_from_settings() {
        let mut root = Root::new("bulk");
        root.0.bulk = true;
        let root = &root.0;
        let settings = root.path.join("sunrise-mods/.sunrise-mod-data");
        fs::create_dir_all(settings.join("fd")).unwrap();
        fs::write(settings.join("fd/enabled"), b"true").unwrap();
        as_mod("fd", || {
            for name in ["z.bin", "a.bin", "m.bin", "other/secret.bin"] {
                write(root, &path(name), b"tile:42").unwrap();
            }
            assert_eq!(read(root, &path("a.bin")).unwrap(), b"tile:42");
            let first = list(root, &ConfigPath::root(), None, 2).unwrap();
            assert_eq!(
                first.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
                ["a.bin", "m.bin"]
            );
            assert_eq!(first[0].stat.len, 7);
            assert_eq!(first[0].stat.kind, ConfigEntryKind::File);
            let second = list(root, &ConfigPath::root(), Some("m.bin"), 2).unwrap();
            assert_eq!(
                second.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
                ["other", "z.bin"]
            );
            assert!(
                list(root, &ConfigPath::root(), Some("z.bin"), 2)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(stat(root, &path("a.bin")).unwrap().unwrap().len, 7);
            assert_eq!(
                list(root, &ConfigPath::root(), None, 0),
                Err(ConfigFileError::InvalidPage)
            );
            assert_eq!(
                list(root, &ConfigPath::root(), None, 1025),
                Err(ConfigFileError::InvalidPage)
            );
            assert_eq!(
                list(root, &ConfigPath::root(), Some("../fd"), 2),
                Err(ConfigFileError::InvalidPage)
            );
            let captured = sunrise_world::native_load_context::capture_settings(&settings, ["fd"]);
            assert_eq!(
                captured["fd"]
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect::<Vec<_>>(),
                [("enabled", "true")]
            );
        });
        as_mod("other", || {
            assert_eq!(read(root, &path("a.bin")), Err(ConfigFileError::NotFound));
            assert_eq!(
                read(root, &path("fd/a.bin")),
                Err(ConfigFileError::NotFound)
            );
            write(root, &path("a.bin"), b"other's tile").unwrap();
            assert_eq!(read(root, &path("a.bin")).unwrap(), b"other's tile");
        });
        assert_eq!(
            fs::read(root.path.join("sunrise-mod-data/fd/a.bin")).unwrap(),
            b"tile:42"
        );
        assert_eq!(
            read(root, &path("a.bin")),
            Err(ConfigFileError::NoCallingMod)
        );
    }

    #[test]
    fn bulk_cap_exceeds_config_and_refuses_without_partial_replacement() {
        let mut root = Root::new("bulk-cap");
        root.0.bulk = true;
        let root = &root.0;
        as_mod("fd", || {
            let larger_than_config = vec![3; (CONFIG_FILE_MAX + 1) as usize];
            write(root, &path("tile"), &larger_than_config).unwrap();
            assert_eq!(read(root, &path("tile")).unwrap(), larger_than_config);
            write(root, &path("tile"), b"keep").unwrap();
            let over = vec![0; DATA_FILE_MAX as usize + 1];
            assert_eq!(
                write(root, &path("tile"), &over),
                Err(ConfigFileError::TooLarge)
            );
            assert_eq!(read(root, &path("tile")).unwrap(), b"keep");
            assert_eq!(
                write(root, &path("absent"), &over),
                Err(ConfigFileError::TooLarge)
            );
            assert_eq!(stat(root, &path("absent")), Ok(None));
            assert_eq!(list(root, &ConfigPath::root(), None, 10).unwrap().len(), 1);
            fs::File::create(root.path.join("sunrise-mod-data/fd/oversized"))
                .unwrap()
                .set_len(DATA_FILE_MAX + 1)
                .unwrap();
            assert_eq!(
                read(root, &path("oversized")),
                Err(ConfigFileError::TooLarge)
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn bulk_links_cannot_reach_another_mod_or_an_external_folder() {
        let mut root = Root::new("bulk-links");
        root.0.bulk = true;
        let outside = Root::new("bulk-links-outside");
        fs::write(outside.0.path.join("secret"), b"outside").unwrap();
        as_mod("other", || {
            write(&root.0, &path("secret"), b"other").unwrap()
        });
        fs::create_dir(root.0.path.join("sunrise-mod-data/fd")).unwrap();
        std::os::unix::fs::symlink("../other", root.0.path.join("sunrise-mod-data/fd/other"))
            .unwrap();
        std::os::unix::fs::symlink(
            &outside.0.path,
            root.0.path.join("sunrise-mod-data/fd/outside"),
        )
        .unwrap();
        as_mod("fd", || {
            for name in ["other/secret", "outside/secret"] {
                assert_eq!(read(&root.0, &path(name)), Err(ConfigFileError::Symlink));
                assert_eq!(
                    write(&root.0, &path(name), b"bad"),
                    Err(ConfigFileError::Symlink)
                );
                assert_eq!(stat(&root.0, &path(name)), Err(ConfigFileError::Symlink));
            }
            assert!(
                list(&root.0, &ConfigPath::root(), None, 10)
                    .unwrap()
                    .is_empty()
            );
        });
        assert_eq!(fs::read(outside.0.path.join("secret")).unwrap(), b"outside");
        assert_eq!(
            fs::read(root.0.path.join("sunrise-mod-data/other/secret")).unwrap(),
            b"other"
        );
    }

    #[test]
    #[ignore = "daily"]
    fn world_bulk_files_survive_save_reload_backup_and_world_deletion() {
        use crate::game::native_entity_refs::tests::{run_fresh_cases, with_game};
        use crate::game::world_loading::WorldRequest;
        if run_fresh_cases(
            "game::server_side::native_config_files::tests::world_bulk_files_survive_save_reload_backup_and_world_deletion",
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
            let rules = std::sync::Arc::new(sunrise_game_data::gamerules::RuleRegistry::vanilla());
            let options = sunrise_sim::world_options::WorldOptions {
                world_type: sunrise_sim::save::WorldPreset::Flat,
                rules: sunrise_game_data::gamerules::GameRules::with_registry(rules.clone(), &[]),
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
            game.side.world_root.clear();
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
                let settings = sunrise_gameplay_common::game::mod_data_root();
                fs::create_dir_all(settings.join("fd")).unwrap();
                fs::write(settings.join("fd/enabled"), b"true").unwrap();
                WARNINGS.store(0, Ordering::Relaxed);
                let captured =
                    sunrise_world::native_load_context::capture_settings(&settings, ["fd"]);
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
                crate::game::ServerGame::data_files()
                    .read(&path("server-cache"))
                    .unwrap(),
                b"game-cache"
            );
        });
    }
}

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
