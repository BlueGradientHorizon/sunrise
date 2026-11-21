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
    use mod_api::{ClientHost, ClientLevelLoadEvent, ClientLevelUnloadEvent, Hooks, ModEntry};
    use std::sync::{Arc, Mutex};
    use sunrise_client::{
        level_loading::Reason,
        net::{Event, ingress},
    };
    use sunrise_protocol::packets::play::SpawnInfo;

    struct Log(Arc<Mutex<Vec<String>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Log {
        fn client_level_load(&self, _: &mut dyn ClientHost, event: &mut ClientLevelLoadEvent<'_>) {
            self.0
                .lock()
                .unwrap()
                .push(format!("load:{}", event.dimension));
        }
        fn client_level_unload(
            &self,
            _: &mut dyn ClientHost,
            event: &mut ClientLevelUnloadEvent<'_>,
        ) {
            self.0
                .lock()
                .unwrap()
                .push(format!("unload:{}", event.dimension));
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
                game.enter_remote_dimension(&login, RemoteDimensionEntry::Login, Reason::Other);
                game.enter_remote_dimension(&login, RemoteDimensionEntry::Respawn, Reason::Other);
                let mut nether = login.clone();
                nether.dimension = "minecraft:the_nether".into();
                game.enter_remote_dimension(&nether, RemoteDimensionEntry::Respawn, Reason::Other);
                let (tx, rx) = ingress::channel(1024, |_| 1).unwrap();
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
    assert_eq!(
        *log.lock().unwrap(),
        [
            "load:minecraft:overworld",
            "load:minecraft:the_nether",
            "unload:minecraft:overworld",
            "unload:minecraft:the_nether",
            "load:minecraft:overworld",
            "unload:minecraft:overworld",
            "load:minecraft:overworld",
            "unload:minecraft:overworld",
        ]
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
