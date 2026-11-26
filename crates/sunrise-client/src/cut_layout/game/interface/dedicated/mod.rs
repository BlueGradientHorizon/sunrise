

#[cfg(test)]
mod language_tests;

#[cfg(test)]
pub(crate) mod tests {

use super::{
        FORWARDING_REMOVED, capture_log, log, log_from, overload_warning, panic_message,
        refuse_forwarding_secret,
    };

use std::{
        fs,
        io::{self, BufRead, BufReader, Read, Write},
        process::{Command, Stdio},
        time::{SystemTime, UNIX_EPOCH},
    };

#[test]
    #[ignore = "daily"]
    fn dedicated_checkpoints_continue_only_after_record_refusals() {
        use {sunrise_gameplay_common::game::execution_role::ExecutionRole, sunrise_gameplay::game::native_entity_refs::tests::fixture_root, sunrise_gameplay::game::native_entity_refs::tests::with_game_role};
        use mod_api::{Authority, Hooks, ModEntry, ServerHost, WorldLifecycleEvent, WorldPhase};
        use std::sync::{Arc, Mutex};
        use sunrise_core::nbt::{DISK_WRITE_NESTING_GUARD, Tag};
        use sunrise_sim::save::{LevelShape, SaveDir};

        struct CloseEvents(Arc<Mutex<Vec<(WorldPhase, String)>>>);
        #[mod_api::__subscriptions]
        impl Hooks for CloseEvents {
            fn world_lifecycle(&self, _: &mut dyn ServerHost, event: &mut WorldLifecycleEvent<'_>) {
                if matches!(
                    event.phase,
                    WorldPhase::WorldCloseFailed | WorldPhase::WorldCloseFinal
                ) {
                    self.0
                        .lock()
                        .unwrap()
                        .push((event.phase, event.error_text.to_owned()));
                }
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![ModEntry::new(
            "crash_close_events",
            Box::new(CloseEvents(events.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_game_role(fixture_root(), ExecutionRole::DedicatedServer, |game| {
                game.native_lifecycle.begin_open(Authority::DedicatedServer);
                game.native_lifecycle.world_open_ready().unwrap();
                let dir = tempfile::tempdir().unwrap();
                game.side.world_root = dir.path().to_path_buf();
                game.side.save.0 = Arc::new(SaveDir::new(
                    game.side.world_root.clone(),
                    LevelShape::OVERWORLD,
                    Some(Default::default()),
                ));
                assert_eq!(
                    game.save_dedicated_checkpoint("startup fixture").unwrap(),
                    0
                );
                let fail_next_save = |game: &mut sunrise_gameplay::game::ServerGame<'_>| {
                    let key = (2, 0);
                    let mut chunk =
                        sunrise_level::chunk::Chunk::with_layout(key.0, key.1, game.world.layout());
                    let invalid = (0..=DISK_WRITE_NESTING_GUARD)
                        .fold(Tag::Int(0), |nested, _| Tag::List(vec![nested]));
                    Arc::make_mut(chunk.saved.get_or_insert_with(Default::default))
                        .root
                        .insert("test:invalid".into(), invalid);
                    game.world.chunks.insert(key, Arc::new(chunk));
                    game.world.modified.insert(key);
                };
                for phase in ["startup fixture", "pause fixture"] {
                    fail_next_save(game);
                    assert_eq!(game.save_dedicated_checkpoint(phase).unwrap(), 1);
                }
                assert_eq!(game.side.save.0.record_save_failures(), 2);
                assert_eq!(game.save_dedicated_checkpoint("no new edits").unwrap(), 0);

                fail_next_save(game);
                let cause = "injected dedicated crash";
                let error = game.stop_after_crash(cause).unwrap_err();
                assert!(error.to_string().contains(cause));
                {
                    let events = events.lock().unwrap();
                    assert_eq!(events.len(), 1);
                    assert_eq!(events[0].0, WorldPhase::WorldCloseFailed);
                    assert!(events[0].1.contains(cause));
                }
                assert_eq!(game.side.save.0.record_save_failures(), 3);

                let blocked = dir.path().join("level.dat_new");
                std::fs::create_dir(&blocked).unwrap();
                let expected = std::fs::write(blocked, b"test").unwrap_err();
                let error = game.save_dedicated_checkpoint("pause fixture").unwrap_err();
                assert_eq!(
                    error.downcast_ref::<std::io::Error>().unwrap().kind(),
                    expected.kind()
                );
                game.native_lifecycle.begin_open(Authority::DedicatedServer);
                game.native_lifecycle.world_open_ready().unwrap();
                let error = game.stop_after_crash(cause).unwrap_err();
                assert!(error.to_string().contains(cause));
                let events = events.lock().unwrap();
                assert_eq!(events.len(), 2);
                assert_eq!(events[1].0, WorldPhase::WorldCloseFailed);
                assert!(events[1].1.contains(cause));
            });
        });
    }
#[test]
    #[ignore = "daily"]
    fn broadcast_console_destination_follows_role_not_launch_marker() {
        use sunrise_gameplay_common::game::execution_role::ExecutionRole;
        let mut resources = None;
        let mut game = sunrise_gameplay::game::game_tests::tests::server_game(&mut resources);
        game.options.dedicated = Some(Vec::new());
        for (role, expected) in [
            (
                ExecutionRole::DedicatedServer,
                vec!["destination probe".to_owned()],
            ),
            (ExecutionRole::IntegratedServer, Vec::new()),
            (ExecutionRole::Client, Vec::new()),
        ] {
            game.options.execution_role = role;
            let lines = capture_log(|| {
                game.native_broadcast(sunrise_game_data::text::Text::literal("destination probe"))
            });
            assert_eq!(lines, expected, "{role:?}");
        }
    }
#[test]
    #[ignore = "daily"]
    fn console_name_lookup_skips_fake_players() {
        sunrise_gameplay::game::native_guest_turn_tests::with_guests(|game| {
            let level = game.level("minecraft:overworld").unwrap();
            game.fake_player(level, 100, "robot").unwrap();
            assert!(game.online_player("robot").is_none());
            assert_eq!(game.online_player("GUEST12").unwrap().id, 12);
            // A fake with the same name must not hide a real player later in join order.
            let guests = &mut game.side.lan.as_mut().unwrap().hub.guests;
            let index = guests.iter().position(|g| g.uuid == 12).unwrap();
            guests.update(index, |g| g.name = "robot".into());
            guests.reverse();
            assert_eq!(game.online_player("ROBOT").unwrap().id, 12);
        });
    }
#[test]
    #[ignore = "daily"]
    fn console_and_management_ip_ban_sets_skip_fake_players() {
        sunrise_gameplay::game::native_guest_turn_tests::with_guests(|game| {
            let level = game.level("minecraft:overworld").unwrap();
            game.fake_player(level, 100, "robot").unwrap();
            assert!(game.ip_ban_players("0.0.0.0").is_empty());
            assert_eq!(game.ip_ban_players("127.0.0.1"), [12, 13]);
            // Even with a matching real peer, the fake is not counted as affected.
            game.side
                .lan
                .as_mut()
                .unwrap()
                .hub
                .update_guest(12, |guest| guest.set_address("0.0.0.0:1".parse().unwrap()))
                .unwrap();
            assert_eq!(game.ip_ban_players("0.0.0.0"), [12]);
            assert_eq!(game.ip_ban_players("127.0.0.1"), [13]);
        });
    }
#[test]
    #[ignore = "daily"]
    fn a_missing_level_file_keeps_the_existing_world_generator() {
        use sunrise_java_properties::Properties;
        use sunrise_sim::save::Level;

        let world = tempfile::tempdir().unwrap();
        let dir = world.path();
        Level {
            seed: 12345,
            ..Level::default()
        }
        .write_without_player(dir)
        .unwrap();
        let settings_path = dir.join("data/minecraft/world_gen_settings.dat");
        assert!(settings_path.is_file(), "test world has no saved generator");
        fs::rename(dir.join("level.dat"), dir.join("level.dat_old")).unwrap();
        let mut properties = Properties::default();
        properties.put("level-seed", "54321".into());
        let props =
            super::ServerProperties::new(properties, &mut super::SystemEntropy::default()).unwrap();
        let resources = std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap().into();
        sunrise_gameplay::game::native_entity_refs::tests::with_game_role(
            resources,
            sunrise_gameplay_common::game::execution_role::ExecutionRole::DedicatedServer,
            |game| {
                game.prepare_world_metadata(dir, &props, false).unwrap();
            },
        );
        let generation = sunrise_sim::save::read_dat(&settings_path).unwrap();
        assert_eq!(
            generation.get("data").and_then(|data| data.get("seed")),
            Some(&sunrise_core::nbt::Tag::Long(12345)),
        );
    }
}


#[cfg(test)]
mod console_source_tests {
    use {sunrise_gameplay_common::game::Options, sunrise_gameplay_common::game::Progress, sunrise_gameplay_common::game::Settings, sunrise_gameplay_common::game::load_resources};
    use std::sync::Arc;
    use sunrise_core::position::ChunkPos;
    use sunrise_level::chunk::Chunk;

    /// Runs `command` as the console does and returns whether it worked and every message it printed.
    fn console(game: &mut sunrise_gameplay::game::ServerGame<'_>, command: &str) -> (bool, String) {
        game.side.chat_capture = Some(Vec::new());
        let ok = game.as_console_in_spawn_level(|g| g.run_command(command));
        let said = game.side.chat_capture.take().unwrap_or_default();
        let said = said
            .iter()
            .map(|t| t.component().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        (ok, said)
    }

    /// `MinecraftServer.createCommandSourceStack`: the console's source has no entity and is named "Server"
    /// (an RCON client's, `RconConsoleSource`, "Rcon"): `/me` says so, and `teammsg`, which needs an entity,
    /// refuses it (`permissions.requires.entity`) instead of running as the player in place.
    #[test]
    #[ignore = "daily"]
    fn the_console_runs_as_the_server_not_the_player_in_place() {
        let root = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES").expect("prepared resources required"),
        );
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            sunrise_gameplay_common::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = sunrise_gameplay_common::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = sunrise_gameplay::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
            crate::game::profile_resolver,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        let (ok, said) = console(&mut game, "me waves");
        assert!(
            ok && said.contains("chat.type.emote") && said.contains("\"Server\""),
            "{said}"
        );
        game.side.rcon_source = true;
        let (ok, said) = console(&mut game, "me waves");
        assert!(ok && said.contains("\"Rcon\""), "{said}");
        game.side.rcon_source = false;
        let (ok, said) = console(&mut game, "teammsg hi");
        assert!(
            !ok && said.contains("permissions.requires.entity"),
            "{said}"
        );
        // The console's source is put back after each line.
        assert!(game.side.command_source.is_none());
    }
}
use sunrise_gameplay_common::game::chat_common::ChatCommonGameExt1 as _;

pub(crate) mod report;

mod missing_mods;

mod management;
