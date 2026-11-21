use crate::game::remote::client_has_level;

/// Minecraft.tick: CLIENT_TICK_END only while a level exists. None before
/// the first Login (configuration has finished, the server has not placed
/// the player yet), and none from StartConfiguration until the next Login.
#[test]
fn tick_ends_need_the_level_the_login_makes() {
    assert!(!client_has_level(false, None), "joined, waiting for Login");
    assert!(client_has_level(true, None), "after Login");
    assert!(!client_has_level(true, Some(40)), "reconfiguring");
}

#[test]
#[ignore = "daily"]
fn client_level_notices_pair_across_replacement_reconfiguration_leave_and_quit() {
    use crate::game::{
        native_client_session::tests::logged_client,
        native_entity_refs::tests::with_client_game,
        remote::{Received, RemoteDimensionEntry},
    };
    use mod_api::{
        BlockState, ClientHost, ClientLevelLoadEvent, ClientLevelUnloadEvent, DatapackCandidate,
        DatapackField, DatapackFieldKind, DatapackReadError, DatapackRegistrySet,
        DatapackRegistrySync, DatapackValue, Hooks, LevelRef, ModEntry, Registrar,
        RegistrationContext,
    };
    use std::sync::{Arc, Mutex};
    use sunrise_client::{
        datapack_net,
        level_loading::Reason,
        net::{Event, ingress},
    };
    use sunrise_protocol::packets::{
        configuration::CustomPayload,
        play::{Clientbound, SpawnInfo},
    };

    struct Notice {
        kind: &'static str,
        dimension: String,
        level: LevelRef,
        block: Option<BlockState>,
        power: Option<i64>,
    }
    struct Log(Arc<Mutex<Vec<Notice>>>);
    impl Log {
        fn record(&self, kind: &'static str, host: &dyn ClientHost, dimension: &str) {
            let level = host
                .level(dimension)
                .expect("the event's level must resolve");
            let block = level.block_state(host, [3, 70, 5]);
            let power = level
                .datapack_handle(host, "levels:spell", "addon:fire")
                .ok()
                .and_then(|handle| {
                    let mut power = None;
                    level
                        .datapack_read(host, &handle, &mut |entry| {
                            if let Some(DatapackValue::Int(value)) = entry.get("power") {
                                power = Some(*value);
                            }
                        })
                        .ok()?;
                    power
                });
            self.0.lock().unwrap().push(Notice {
                kind,
                dimension: dimension.to_owned(),
                level,
                block,
                power,
            });
        }
    }
    #[mod_api::__subscriptions]
    impl Hooks for Log {
        fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
            registrar
                .declare_datapack_registry(
                    "spell",
                    DatapackRegistrySync::Required,
                    vec![DatapackField::required(
                        "power",
                        DatapackFieldKind::Int { min: 0, max: 100 },
                    )],
                )
                .unwrap();
        }
        fn client_level_load(
            &self,
            host: &mut dyn ClientHost,
            event: &mut ClientLevelLoadEvent<'_>,
        ) {
            self.record("load", host, event.dimension);
        }
        fn client_level_unload(
            &self,
            host: &mut dyn ClientHost,
            event: &mut ClientLevelUnloadEvent<'_>,
        ) {
            self.record("unload", host, event.dimension);
        }
    }
    let log = Arc::new(Mutex::new(Vec::new()));
    mod_platform::testing::with_mods(
        vec![ModEntry::new("levels", Box::new(Log(log.clone())))].into(),
        || {
            with_client_game(|game| {
                let login = SpawnInfo {
                    dimension_type: 0,
                    dimension: "minecraft:overworld".into(),
                    seed: 0,
                    game_type: 1,
                    previous_game_type: None,
                    debug: false,
                    flat: true,
                    last_death: None,
                    portal_cooldown: 0,
                    sea_level: 63,
                };
                let _server = logged_client(game);
                let declarations = mod_api::__private::datapack_registry_declarations();
                let candidate = |power| {
                    DatapackCandidate::build(
                        declarations,
                        &[(
                            "data/addon/levels/spell/fire.json".into(),
                            vec![(
                                "fixture".into(),
                                format!(r#"{{"power":{power}}}"#).into_bytes(),
                            )],
                        )],
                    )
                    .unwrap()
                };
                let encode = |set: &DatapackRegistrySet| {
                    set.snapshot(declarations, |_| true).encode().unwrap()
                };
                let receiver = |set: &DatapackRegistrySet| {
                    let mut receiver = datapack_net::Receiver::default();
                    for chunk in datapack_net::chunks(&encode(set)).unwrap() {
                        receiver.accept_chunk(&chunk).unwrap();
                    }
                    receiver
                };
                game.remote.as_mut().unwrap().datapacks =
                    receiver(&DatapackRegistrySet::first(candidate(17), 900));
                game.enter_remote_dimension(&login, RemoteDimensionEntry::Login, Reason::Other);
                let overworld = game.level(&login.dimension).unwrap();
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
                game.enter_remote_dimension(&login, RemoteDimensionEntry::Respawn, Reason::Other);
                assert_eq!(game.level(&login.dimension), Some(overworld));
                let mut nether = login.clone();
                nether.dimension = "minecraft:the_nether".into();
                game.enter_remote_dimension(&nether, RemoteDimensionEntry::Respawn, Reason::Other);
                let nether = game.level(&nether.dimension).unwrap();
                assert_eq!(
                    log.lock().unwrap()[1].level,
                    nether,
                    "constructor handle survives installation"
                );
                assert_eq!(
                    game.level_block_state(overworld, [3, 70, 5]),
                    None,
                    "old handle retires after Unload"
                );
                assert_eq!(
                    overworld.datapack_entries(game, "levels:spell"),
                    None,
                    "a synchronized registry does not revive a stale level handle"
                );
                let old_entry = nether
                    .datapack_handle(game, "levels:spell", "addon:fire")
                    .unwrap();
                let next_set = DatapackRegistrySet::first(candidate(29), 901);
                let (tx, rx) = ingress::channel(1024, |_| 1).unwrap();
                tx.send(Event::Reconfiguring).unwrap();
                // The next configuration has already accepted all its registry fragments
                // and queued the handoff before the game can unload the retiring level.
                tx.send(Event::Reconfigured {
                    registries: game.remote.as_ref().unwrap().registries.clone(),
                    datapacks: receiver(&next_set),
                })
                .unwrap();
                game.remote
                    .as_mut()
                    .unwrap()
                    .backlog
                    .extend(rx.take_turn().into_iter().map(|receipt| {
                        let (event, credit) = receipt.into_parts();
                        Received::Event(event, credit)
                    }));
                game.poll_remote_events(true).unwrap();
                game.enter_remote_dimension(&login, RemoteDimensionEntry::Login, Reason::Other);
                let next = game.level(&login.dimension).unwrap();
                assert_eq!(nether.datapack_entries(game, "levels:spell"), None);
                assert_eq!(
                    next.datapack_read(game, &old_entry, &mut |_| unreachable!()),
                    Err(DatapackReadError::WrongSession),
                );
                let next_entry = next
                    .datapack_handle(game, "levels:spell", "addon:fire")
                    .unwrap();
                let reloaded = next_set.reload(candidate(31)).unwrap();
                for data in datapack_net::chunks(&encode(&reloaded)).unwrap() {
                    game.apply_packet(Clientbound::CustomPayload(CustomPayload {
                        channel: datapack_net::CHANNEL.into(),
                        data,
                    }))
                    .unwrap();
                }
                assert_eq!(
                    next.datapack_read(game, &next_entry, &mut |_| unreachable!()),
                    Err(DatapackReadError::StaleGeneration),
                );
                tx.send(Event::Reconfiguring).unwrap();
                let (event, credit) = rx.try_recv_owned().unwrap().into_parts();
                game.remote
                    .as_mut()
                    .unwrap()
                    .backlog
                    .push_back(Received::Event(event, credit));
                game.poll_remote_events(true).unwrap();
                // Disconnecting from the reconfiguration screen must not unload twice.
                game.leave_server(None).unwrap();
                let _server = logged_client(game);
                game.enter_remote_dimension(&login, RemoteDimensionEntry::Login, Reason::Other);
                game.leave_server(None).unwrap();
                let _server = logged_client(game);
                game.enter_remote_dimension(&login, RemoteDimensionEntry::Login, Reason::Other);
                game.begin_closing_save(true).unwrap();
                assert!(game.poll_closing_save().unwrap());
                game.save_for_exit().unwrap();
            });
        },
    );
    let log = log.lock().unwrap();
    assert_eq!(
        log.iter()
            .map(|notice| (notice.kind, notice.dimension.as_str()))
            .collect::<Vec<_>>(),
        [
            ("load", "minecraft:overworld"),
            ("load", "minecraft:the_nether"),
            ("unload", "minecraft:overworld"),
            ("unload", "minecraft:the_nether"),
            ("load", "minecraft:overworld"),
            ("unload", "minecraft:overworld"),
            ("load", "minecraft:overworld"),
            ("unload", "minecraft:overworld"),
            ("load", "minecraft:overworld"),
            ("unload", "minecraft:overworld"),
        ]
    );
    assert_eq!(log[0].level, log[2].level);
    assert_eq!(log[1].level, log[3].level);
    assert_ne!(log[0].level.epoch(), log[1].level.epoch());
    assert!(log[0].block.is_none() && log[1].block.is_none());
    assert_eq!(
        log.iter().map(|notice| notice.power).collect::<Vec<_>>(),
        [
            Some(17),
            Some(17),
            Some(17),
            Some(17),
            Some(29),
            Some(31),
            None,
            None,
            None,
            None,
        ],
        "retiring levels see their own configuration and the new level sees its handoff and reload"
    );
    assert!(
        log[2].block.is_some(),
        "the retiring Overworld still holds the old chunk"
    );
}

/// #12148: a proxy moves the player to its next server. The last server's
/// columns stay until the next Login, but Reconfigured already put in the
/// next server's biome table, shorter when only the last server had a
/// biome the game lacks: the columns hold an id past it ("the len is 67
/// but the index is 67"). Vanilla's clearClientLevel leaves no player, so
/// getSituationalMusic is MENU, and ServerReconfigScreen (no
/// LevelLoadingScreen) lets MusicManager.tick start it.
#[test]
#[ignore = "daily"]
fn a_reconfiguring_client_plays_menu_music_and_never_indexes_past_the_biomes() {
    use crate::game::{Audio, Game, Options, Progress, Settings, load_resources};
    use sunrise_client::{chunk::Chunk, music::Music};
    use sunrise_core::geometry::Vec3;
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
        crate::game::execution_role::ExecutionRole::Client,
        &Progress::new(),
        &mut |_, _| unreachable!(),
    )
    .unwrap();
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.clone();
    let mut game = Game::new(
        &loaded.resources,
        loaded.pack,
        loaded.gui.unwrap(),
        options,
        settings,
        Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
            root.clone(),
        )),
        None,
    )
    .unwrap();
    game.init_remote_for_replay().unwrap();
    // The last server's column at the origin: its extra biome came after
    // the built-in ones, one past the table the next server leaves.
    let stale = world_biomes!(game).list.len() as u16;
    let mut column = Chunk::empty(0, 0);
    // Quart 16 above the column's bottom at -64: block y 0.
    column.set_biome_cell(0, 16, 0, stale);
    game.world
        .chunks
        .insert((0, 0), std::sync::Arc::new(column));
    assert_eq!(game.biome_at([0.0; 3]), "minecraft:plains");
    assert_eq!(
        game.biome_name_at(Vec3::new(0.5, 0.5, 0.5)).as_deref(),
        Some("minecraft:plains")
    );

    // StartConfiguration after a Login: no player, ServerReconfigScreen.
    let ticks = game.ticks;
    let r = game.remote.as_mut().unwrap();
    r.logged_in = true;
    r.reconfiguring = Some(ticks);
    game.player = None;
    assert_eq!(game.situational_music().0, Some(Music::menu()));
    // A new manager waits its first 100 ticks, then starts the menu track.
    let menu = (0..=100).any(|_| {
        game.tick_music();
        game.music.current(&game.audio.engine) == Some("minecraft:music.menu")
    });
    assert!(menu, "the menu music starts under ServerReconfigScreen");

    // With a level again the camera probe reads that column: plains' music.
    game.remote.as_mut().unwrap().reconfiguring = None;
    let (music, _) = game.situational_music();
    assert!(music.is_some_and(|m| m != Music::menu()));
}
