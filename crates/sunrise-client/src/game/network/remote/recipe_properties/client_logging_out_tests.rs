use crate::game::native_entity_refs::tests::with_client_game;
use crate::game::native_lifecycle::NativeLifecycle;
use crate::game::remote::RemoteDimensionEntry;
use mod_api::{
    ClientHost, ClientLevelUnloadEvent, ClientPlayerLoggingOutEvent, GameMode, Hooks, ModEntry,
    PlayerRef,
};
use mod_platform::testing::with_mods;
use std::sync::{Arc, Mutex};
use sunrise_client::level_loading::Reason;
use sunrise_protocol::{
    connection::Pipe,
    packets::play::{self, Clientbound as C, SpawnInfo},
};

#[derive(Debug, PartialEq, Eq)]
enum Observed {
    Logout {
        player: Option<PlayerRef>,
        game_mode: Option<GameMode>,
        local_player: Option<PlayerRef>,
        host_game_mode: Option<GameMode>,
        level_live: bool,
    },
    LevelUnload(String),
}

struct Probe {
    events: Arc<Mutex<Vec<Observed>>>,
    expected_dimension: Arc<Mutex<String>>,
}

#[mod_api::__subscriptions]
impl Hooks for Probe {
    fn client_player_logging_out(
        &self,
        host: &mut dyn ClientHost,
        event: &mut ClientPlayerLoggingOutEvent,
    ) {
        let dim = self.expected_dimension.lock().unwrap().clone();
        let local_player = host.local_player();
        let host_game_mode = event.player.and_then(|player| player.game_mode(&*host));
        let level_live = host.level(&dim).is_some();
        self.events.lock().unwrap().push(Observed::Logout {
            player: event.player,
            game_mode: event.game_mode,
            local_player,
            host_game_mode,
            level_live,
        });
    }

    fn client_level_unload(
        &self,
        _host: &mut dyn ClientHost,
        event: &mut ClientLevelUnloadEvent<'_>,
    ) {
        self.events
            .lock()
            .unwrap()
            .push(Observed::LevelUnload(event.dimension.to_owned()));
    }
}

fn spawn_info(dimension: &str, game_type: i32) -> SpawnInfo {
    SpawnInfo {
        dimension_type: 0,
        dimension: dimension.into(),
        seed: 0,
        game_type,
        previous_game_type: None,
        debug: false,
        flat: true,
        last_death: None,
        portal_cooldown: 0,
        sea_level: 63,
    }
}

#[test]
#[ignore = "daily"]
fn client_player_logging_out_fires_once_before_teardown_and_handles_no_player_transitions() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let expected_dimension = Arc::new(Mutex::new("minecraft:overworld".to_owned()));
    let probe = Probe {
        events: Arc::clone(&events),
        expected_dimension: Arc::clone(&expected_dimension),
    };
    let mods: mod_api::Mods = vec![ModEntry::new("logout_probe", Box::new(probe))].into();
    with_mods(mods, || {
        with_client_game(|game| {
            let take = || std::mem::take(&mut *events.lock().unwrap());

            // 1. No-player disconnect transition with no remote session
            // (Minecraft.disconnect from title / doWorldLoad / ConnectScreen.startConnecting).
            assert!(game.remote.is_none());
            game.client_logging_out();
            assert_eq!(
                take(),
                vec![Observed::Logout {
                    player: None,
                    game_mode: None,
                    local_player: None,
                    host_game_mode: None,
                    level_live: false,
                }]
            );

            // 2. Pre-login disconnect: remote exists, but no player or level yet.
            let (client, _server) = Pipe::pair();
            game.connect_local(client, "PreLogin").unwrap();
            game.leave_server(None).unwrap();
            assert_eq!(
                take(),
                vec![Observed::Logout {
                    player: None,
                    game_mode: None,
                    local_player: None,
                    host_game_mode: None,
                    level_live: false,
                }]
            );

            // 3. Logged-in disconnect via leave_server: fires once before teardown
            // with live PlayerRef and GameMode; dimension change (Respawn) does not fire it.
            game.native_lifecycle = NativeLifecycle::default();
            let (client, _server) = Pipe::pair();
            game.connect_local(client, "LoggedIn").unwrap();
            game.enter_remote_dimension(
                &spawn_info("minecraft:overworld", GameMode::Survival.id()),
                RemoteDimensionEntry::Login,
                Reason::Other,
            );
            {
                let remote = game.remote.as_mut().unwrap();
                remote.logged_in = true;
                remote.entity_id = 42;
                remote.profile = 0xDEAD_BEEF;
            }
            take();

            let nether = spawn_info("minecraft:the_nether", GameMode::Creative.id());
            *expected_dimension.lock().unwrap() = "minecraft:the_nether".to_owned();
            game.handle_packet(C::Respawn(play::Respawn {
                spawn: nether,
                keep: 0,
            }))
            .unwrap();
            assert_eq!(
                take(),
                vec![Observed::LevelUnload("minecraft:overworld".to_owned())],
                "dimension change unloads the old level without firing client_player_logging_out"
            );

            let live_player = game.local_player().expect("logged-in local player");
            game.leave_server(None).unwrap();
            assert_eq!(
                take(),
                vec![
                    Observed::Logout {
                        player: Some(live_player),
                        game_mode: Some(GameMode::Creative),
                        local_player: Some(live_player),
                        host_game_mode: Some(GameMode::Creative),
                        level_live: true,
                    },
                    Observed::LevelUnload("minecraft:the_nether".to_owned()),
                ],
                "logout runs once with live player and mode before level unload and teardown"
            );
            assert!(game.local_player().is_none());
            assert_eq!(live_player.game_mode(game), None);

            // 4. Duplicate logout calls on the same connection fire only once.
            game.native_lifecycle = NativeLifecycle::default();
            let (client, _server) = Pipe::pair();
            game.connect_local(client, "Deduplicated").unwrap();
            *expected_dimension.lock().unwrap() = "minecraft:overworld".to_owned();
            game.enter_remote_dimension(
                &spawn_info("minecraft:overworld", GameMode::Adventure.id()),
                RemoteDimensionEntry::Login,
                Reason::Other,
            );
            {
                let remote = game.remote.as_mut().unwrap();
                remote.logged_in = true;
                remote.entity_id = 77;
                remote.profile = 0xCAFE_BABE;
            }
            take();

            let second_player = game.local_player().expect("second local player");
            game.client_logging_out();
            game.client_logging_out();
            game.leave_server(None).unwrap();
            assert_eq!(
                take(),
                vec![
                    Observed::Logout {
                        player: Some(second_player),
                        game_mode: Some(GameMode::Adventure),
                        local_player: Some(second_player),
                        host_game_mode: Some(GameMode::Adventure),
                        level_live: true,
                    },
                    Observed::LevelUnload("minecraft:overworld".to_owned()),
                ]
            );

            // 5. Disconnect while reconfiguring (clearClientLevel already cleared the player).
            game.native_lifecycle = NativeLifecycle::default();
            let (client, _server) = Pipe::pair();
            game.connect_local(client, "Reconfiguring").unwrap();
            game.enter_remote_dimension(
                &spawn_info("minecraft:overworld", GameMode::Survival.id()),
                RemoteDimensionEntry::Login,
                Reason::Other,
            );
            {
                let remote = game.remote.as_mut().unwrap();
                remote.logged_in = true;
                remote.entity_id = 88;
                remote.profile = 0x1234_5678;
                remote.reconfiguring = Some(1);
                remote.heard_level = None;
            }
            game.native_lifecycle.begin_close();
            take();

            game.leave_server(None).unwrap();
            assert_eq!(
                take(),
                vec![Observed::Logout {
                    player: None,
                    game_mode: None,
                    local_player: None,
                    host_game_mode: None,
                    level_live: false,
                }]
            );
        });
    });
}
