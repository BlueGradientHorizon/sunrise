//! `player_changed_dimension` is heard on every logical server: the integrated server that runs
//! single player and the LAN world opened from it, and a dedicated server (#3250).
//! These prepared tests also exercise menu retirement during travel.
use super::Dim;
use crate::game::{
    Game, execution_role::ExecutionRole, lan::LanSettings, lan_actions::GuestCommand,
    native_entity_refs::tests::with_game_role,
};
use mod_api::{
    Hooks, LevelDimension, ModEntry, PlayerChangedDimensionEvent, PlayerRespawnPositionEvent,
    ServerHost,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI32, AtomicUsize},
};
use sunrise_client::{
    chunk::Chunk,
    lan::{Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig},
};
use sunrise_core::geometry::Vec3;
use sunrise_protocol::packets::configuration::ClientInformation;

const CONN: u32 = 12;
const UUID: u128 = 120;

type Heard = Arc<Mutex<Vec<(String, String)>>>;

struct Recorder(Heard);

#[mod_api::__subscriptions]
impl Hooks for Recorder {
    fn player_changed_dimension(
        &self,
        _: &mut dyn ServerHost,
        event: &mut PlayerChangedDimensionEvent<'_>,
    ) {
        self.0
            .lock()
            .unwrap()
            .push((event.from.to_owned(), event.to.to_owned()));
    }
}

struct MoveRespawn;

#[mod_api::__subscriptions]
impl Hooks for MoveRespawn {
    fn player_respawn_position(
        &self,
        _: &mut dyn ServerHost,
        event: &mut PlayerRespawnPositionEvent,
    ) {
        event.position[0] += 10.0;
    }
}

struct KeepRespawn;

#[mod_api::__subscriptions]
impl Hooks for KeepRespawn {
    fn player_respawn_position(&self, _: &mut dyn ServerHost, _: &mut PlayerRespawnPositionEvent) {}
}

struct CrossLevelRespawn;

#[mod_api::__subscriptions]
impl Hooks for CrossLevelRespawn {
    fn player_respawn_position(
        &self,
        _: &mut dyn ServerHost,
        event: &mut PlayerRespawnPositionEvent,
    ) {
        *event.level = LevelDimension::Nether;
    }
}

/// A server of `role` with one survival player at (3.5, 64, 5.5) in the Overworld, its options as
/// `integrated.rs`'s server Game and a dedicated server set them.
fn with_server(role: ExecutionRole, test: impl FnOnce(&mut Game<'_>)) {
    let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
    with_game_role(root, role, |game| {
        game.options.dedicated = Some(Vec::new());
        let config = HubConfig {
            default_game_mode: 0,
            guest_commands: true,
            force_game_mode: false,
            hardcore: false,
            view_distance: 2,
            simulation_distance: 2,
            dimension: "minecraft:overworld".into(),
            allow_flight: false,
            seed: 0,
            max_players: 2,
            integrated: role == ExecutionRole::IntegratedServer,
            levels: game.level_shapes,
            level_seeds: game.levels.runtime.seeds(),
        };
        game.lan_host(
            LanSettings {
                port: 0,
                guest_commands: true,
                force_game_mode: false,
            },
            NetConfig {
                listen: false,
                announce: false,
                lod_distance: 0,
                ..NetConfig::lan(0, String::new(), 2)
            },
            config,
        )
        .unwrap();
        light(game, 0, 0);
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut guest = Guest::new(
            Joined {
                conn: CONN,
                uuid: UUID,
                name: "Owner".into(),
                properties: Vec::new(),
                info: ClientInformation::new(2),
                brand: "test".into(),
                sender: PlayerSender::new(tx, Arc::new(AtomicUsize::new(0))),
                latency: Arc::new(AtomicI32::new(0)),
                address: "127.0.0.1:1".parse().unwrap(),
                expected: false,
            },
            CONN as i32,
            [3.5, 64.0, 5.5],
            2,
        );
        guest.dimension = "minecraft:overworld".into();
        guest.placed = true;
        guest.loaded = true;
        guest.health = 20.0;
        guest.motion = Some(Box::new(
            sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::new(3.5, 64.0, 5.5))
                .unwrap(),
        ));
        game.lan.as_mut().unwrap().hub.guests.push(guest);
        test(game);
    });
}

/// Lights the 5x5 chunks around chunk (`cx`, `cz`) of the level in place, as its
/// streaming does once they have loaded.
fn light(game: &mut Game<'_>, cx: i32, cz: i32) {
    for x in cx - 2..=cx + 2 {
        for z in cz - 2..=cz + 2 {
            game.streaming
                .insert_remote(&mut game.world, Chunk::empty(x, z));
        }
    }
}

/// Runs the server's travel until the player's trip has ended in `dim`, loading
/// the destination around the trip's target.
fn finish_trip(game: &mut Game<'_>, dim: Dim) {
    let home = game.dim;
    game.enter_level(dim).unwrap();
    let at = game
        .server_trips
        .iter()
        .find(|trip| trip.conn == CONN)
        .expect("the player is on a trip")
        .target;
    light(game, (at.x.floor() as i32) >> 4, (at.z.floor() as i32) >> 4);
    for _ in 0..20 {
        let (target_dim, target) = game
            .server_trips
            .iter()
            .find(|trip| trip.conn == CONN)
            .map(|trip| (trip.dim, trip.target))
            .unwrap_or((game.dim, at));
        if game.dim != target_dim {
            game.enter_level(target_dim).unwrap();
        }
        light(
            game,
            (target.x.floor() as i32) >> 4,
            (target.z.floor() as i32) >> 4,
        );
        if !game.on_server_trip(CONN) {
            break;
        }
        game.tick_server_travel();
    }
    assert!(
        !game.on_server_trip(CONN),
        "the trip into {dim:?} never ended (current {:?}, trips {:?})",
        game.dim,
        game.server_trips
            .iter()
            .map(|trip| (
                trip.dim,
                trip.target,
                trip.respawn.is_some(),
                trip.world_spawn
            ))
            .collect::<Vec<_>>()
    );
    game.enter_level(home).unwrap();
}

fn place_anchor(game: &mut Game<'_>, pos: [i32; 3]) {
    light(game, pos[0] >> 4, pos[2] >> 4);
    let registry = game.pack.assets.registry;
    let id = registry.default_state("minecraft:respawn_anchor").unwrap();
    let mut state = registry.state(id).unwrap().clone();
    state.properties.insert("charges".into(), "1".into());
    let charged = registry.resolve(&state).unwrap();
    game.world.set(pos[0], pos[1], pos[2], charged).unwrap();
}

fn run_as_player(game: &mut Game<'_>, role: ExecutionRole, command: &str) {
    assert!(game.run_guest_command(GuestCommand {
        conn: CONN,
        command: command.into(),
        permission: 4,
        integrated: role == ExecutionRole::IntegratedServer,
        quiet: false,
    }));
    // The integrated server runs them at its tick's start (`integrated.rs`).
    game.run_level_teleports();
}

fn prepare_end_exit(game: &mut Game<'_>, credits: bool) {
    game.enter_level(Dim::End).unwrap();
    game.streaming
        .insert_remote(&mut game.world, Chunk::empty(0, 0));
    assert!(
        game.set_state_named([3, 64, 5], "minecraft:end_portal", &[])
            .unwrap()
    );
    game.lan.as_mut().unwrap().hub.update_guest(CONN, |guest| {
        guest.dimension = Dim::End.id().to_owned();
        guest.position = [3.5, 64.0, 5.5];
        guest.seen_credits = credits;
    });
}

fn heard_on(role: ExecutionRole, test: impl FnOnce(&mut Game<'_>, &Heard)) {
    let heard = Heard::default();
    let mods: mod_api::Mods = vec![ModEntry::new(
        "dimension-recorder",
        Box::new(Recorder(heard.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_server(role, |game| test(game, &heard));
    });
}

const SERVERS: [ExecutionRole; 2] = [
    ExecutionRole::IntegratedServer,
    ExecutionRole::DedicatedServer,
];

fn overworld_to_nether() -> Vec<(String, String)> {
    vec![("minecraft:overworld".into(), "minecraft:the_nether".into())]
}

#[test]
#[ignore = "daily"]
fn a_cross_level_teleport_returns_both_cursors_before_arrival() {
    use sunrise_client::items::ItemStack;

    with_server(ExecutionRole::DedicatedServer, |game| {
        let inventory_cursor = ItemStack::new("minecraft:iron_ingot", 3);
        let open_cursor = ItemStack::new("minecraft:gold_ingot", 5);
        let hub = &mut game.lan.as_mut().unwrap().hub;
        hub.guests
            .update(0, |g| g.carried = Some(inventory_cursor.clone()));
        hub.open_native_menu(
            CONN,
            1,
            sunrise_protocol::text::literal("fixture"),
            Vec::new(),
        )
        .unwrap();
        hub.guests
            .update(0, |g| g.carried = Some(open_cursor.clone()));
        game.level_teleports.push(super::LevelTeleport {
            conn: CONN,
            dim: Dim::Nether,
            at: [8.5, 70.0, 8.5],
            rotation: None,
            vehicle: None,
        });
        game.run_level_teleports();

        let guest = &game.lan.as_ref().unwrap().hub.guests[0];
        assert_eq!(guest.dimension, Dim::Nether.id());
        assert!(guest.menu.is_none());
        assert!(guest.carried.is_none());
        assert_eq!(
            guest
                .inventory
                .iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>(),
            [inventory_cursor, open_cursor]
        );
    });
}

#[test]
#[ignore = "daily"]
fn a_cross_level_teleport_survives_cursor_overflow_removing_an_earlier_guest() {
    use mod_api::{EntityJoinLevelEvent, Flow, PlayerRef};
    use sunrise_client::items::ItemStack;

    struct RemoveFake(Arc<Mutex<Option<PlayerRef>>>);
    #[mod_api::__subscriptions]
    impl Hooks for RemoveFake {
        fn entity_join_level(
            &self,
            host: &mut dyn ServerHost,
            event: &mut EntityJoinLevelEvent<'_>,
        ) -> Flow {
            if event.kind == "minecraft:item"
                && let Some(fake) = self.0.lock().unwrap().take()
            {
                assert!(fake.remove_fake(host));
            }
            Flow::Continue
        }
    }

    let fake = Arc::new(Mutex::new(None));
    let mods = vec![ModEntry::new(
        "remove-fake-on-drop",
        Box::new(RemoveFake(fake.clone())),
    )];
    mod_platform::testing::with_mods(mods.into(), || {
        with_server(ExecutionRole::DedicatedServer, |game| {
            let traveller = game.lan.as_mut().unwrap().hub.guests.remove(0);
            let level = game.level(Dim::Overworld.id()).unwrap();
            *fake.lock().unwrap() = Some(game.fake_player(level, 121, "Automation").unwrap());
            let hub = &mut game.lan.as_mut().unwrap().hub;
            assert!(hub.guests[0].is_fake());
            hub.guests.push(traveller);
            assert_eq!(hub.guests[1].conn, CONN);
            let queued = ItemStack::new("minecraft:iron_ingot", 3);
            let carried = ItemStack::new("minecraft:gold_ingot", 5);
            hub.guests.update(1, |g| {
                g.inventory
                    .fill(Some(ItemStack::new("minecraft:stone", 64)));
            });
            let title = sunrise_protocol::text::literal("fixture");
            hub.open_native_menu(CONN, 1, title.clone(), Vec::new())
                .unwrap();
            hub.guests.update(1, |g| g.carried = Some(queued.clone()));
            hub.close_container(CONN); // A cursor waiting to return when travel starts.
            hub.open_native_menu(CONN, 1, title, Vec::new()).unwrap();
            hub.guests.update(1, |g| g.carried = Some(carried.clone()));

            run_as_player(
                game,
                ExecutionRole::DedicatedServer,
                "execute in minecraft:the_nether run tp @s 8.5 70 8.5",
            );

            let guests = &game.lan.as_ref().unwrap().hub.guests;
            assert_eq!(guests.len(), 1);
            assert_eq!(guests[0].conn, CONN);
            assert_eq!(guests[0].dimension, Dim::Nether.id());
            assert!(guests[0].menu.is_none());
            assert!(guests[0].carried.is_none());
            assert_eq!(game.dim, Dim::Overworld);
            assert_eq!(
                game.dropped
                    .iter()
                    .map(|item| item.stack.clone())
                    .collect::<Vec<_>>(),
                [queued, carried]
            );
        });
    });
}

#[test]
#[ignore = "daily"]
fn a_failed_cross_level_trip_closes_the_retired_client_window() {
    use mod_api::{BlockUpdateFlags, EntityJoinLevelEvent, Flow};
    use sunrise_client::{items::ItemStack, lan::Out};
    use sunrise_protocol::{buf::Reader, ids::play::clientbound};

    const CHEST: [i32; 3] = [4, 64, 5];
    struct OpenOnOverflow;
    #[mod_api::__subscriptions]
    impl Hooks for OpenOnOverflow {
        fn entity_join_level(
            &self,
            host: &mut dyn ServerHost,
            event: &mut EntityJoinLevelEvent<'_>,
        ) -> Flow {
            if event.kind == "minecraft:item" {
                let level = host.level(Dim::Overworld.id()).unwrap();
                let chest = host.block_entity(level, CHEST).unwrap();
                let player = host.player(UUID).unwrap();
                assert_eq!(player.open_menu(host, chest), Ok(true));
            }
            Flow::Continue
        }
    }

    let mods = vec![ModEntry::new("open-on-overflow", Box::new(OpenOnOverflow))];
    mod_platform::testing::with_mods(mods.into(), || {
        with_server(ExecutionRole::DedicatedServer, |game| {
            // Opening an unloaded destination refuses an unreadable saved biome layout.
            // Set a world path: the constructor-only fixture otherwise reads the process's cwd.
            let world = tempfile::tempdir().unwrap();
            game.world_root = world.path().to_path_buf();
            let record = world.path().join("data/sunrise/biome_regions.dat");
            std::fs::create_dir_all(record.parent().unwrap()).unwrap();
            std::fs::write(record, b"not a biome layout").unwrap();
            let level = game.level(Dim::Overworld.id()).unwrap();
            let chest = game
                .pack
                .assets
                .registry
                .default_state("minecraft:chest")
                .unwrap();
            assert_eq!(
                game.level_set_block(level, CHEST, chest, BlockUpdateFlags::NEIGHBORS),
                Ok(true)
            );
            let (tx, rx) = std::sync::mpsc::channel();
            game.lan.as_mut().unwrap().hub.update_guest(CONN, |g| {
                g.sender = PlayerSender::new(tx, Arc::default());
                g.inventory
                    .fill(Some(ItemStack::new("minecraft:stone", 64)));
            });
            for via_portal in [false, true] {
                game.lan.as_mut().unwrap().hub.update_guest(CONN, |g| {
                    assert!(g.menu.is_none());
                    g.carried = Some(ItemStack::new("minecraft:gold_ingot", 5));
                });
                if via_portal {
                    game.start_server_trip(CONN, Dim::Nether, None, false);
                } else {
                    run_as_player(
                        game,
                        ExecutionRole::DedicatedServer,
                        "execute in minecraft:the_nether run tp @s 8.5 70 8.5",
                    );
                }
                let mut windows = Vec::new();
                for packet in rx.try_iter() {
                    if let Out::Packet(packet) = packet {
                        let mut reader = Reader::new(&packet);
                        let id = reader.varint().unwrap();
                        assert_ne!(id, clientbound::RESPAWN);
                        if matches!(id, clientbound::OPEN_SCREEN | clientbound::CONTAINER_CLOSE) {
                            windows.push((id, reader.varint().unwrap()));
                        }
                    }
                }
                assert!(
                    matches!(windows.as_slice(),
                        [(clientbound::OPEN_SCREEN, open), (clientbound::CONTAINER_CLOSE, close)]
                            if open == close && *open != 0
                    ),
                    "via portal: {via_portal}, packets: {windows:?}"
                );
                let guest = game.lan.as_ref().unwrap().hub.guest(CONN).unwrap();
                assert_eq!(guest.dimension, Dim::Overworld.id());
                assert!(guest.menu.is_none());
            }
        });
    });
}

#[test]
#[ignore = "daily"]
fn a_nether_portal_trip_is_heard_once_on_every_server() {
    for role in SERVERS {
        heard_on(role, |game, heard| {
            let portal = game
                .pack
                .assets
                .registry
                .default_state("minecraft:nether_portal")
                .unwrap();
            game.world.set(3, 64, 5, portal).unwrap();
            game.world.set(3, 65, 5, portal).unwrap();
            // `playersNetherPortalDefaultDelay` is 80 ticks for a survival player.
            for _ in 0..100 {
                if game.on_server_trip(CONN) {
                    break;
                }
                game.tick_server_travel();
            }
            assert!(
                game.on_server_trip(CONN),
                "{role:?}: the portal never took the player"
            );
            assert!(
                heard.lock().unwrap().is_empty(),
                "{role:?}: heard before the trip ended"
            );
            finish_trip(game, Dim::Nether);
            assert_eq!(*heard.lock().unwrap(), overworld_to_nether(), "{role:?}");
        });
    }
}

#[test]
#[ignore = "daily"]
fn a_command_teleport_into_the_nether_is_heard_once_on_every_server() {
    for role in SERVERS {
        heard_on(role, |game, heard| {
            run_as_player(
                game,
                role,
                "execute in minecraft:the_nether run tp @s 0 64 0",
            );
            finish_trip(game, Dim::Nether);
            assert_eq!(*heard.lock().unwrap(), overworld_to_nether(), "{role:?}");
        });
    }
}

#[test]
#[ignore = "daily"]
fn player_tick_count_survives_teleports_and_deferred_arrivals() {
    with_server(ExecutionRole::DedicatedServer, |game| {
        let entity = game.entity(CONN).unwrap();
        game.lan.as_mut().unwrap().hub.update_guest(CONN, |guest| {
            guest.motion.as_mut().unwrap().health.tick_count = 17;
        });

        run_as_player(
            game,
            ExecutionRole::DedicatedServer,
            "execute in minecraft:overworld run tp @s 8.5 70 8.5",
        );
        assert_eq!(entity.tick_count(game), Some(17));

        run_as_player(
            game,
            ExecutionRole::DedicatedServer,
            "execute in minecraft:the_nether run tp @s 8.5 70 8.5",
        );
        assert_eq!(entity.tick_count(game), Some(17));
        finish_trip(game, Dim::Nether);
        assert_eq!(entity.tick_count(game), Some(17));

        game.enter_level(Dim::Nether).unwrap();
        game.as_guest(CONN, |game| {
            game.player.as_mut().unwrap().health.tick_count = 29;
        })
        .unwrap();
        run_as_player(
            game,
            ExecutionRole::DedicatedServer,
            "execute in minecraft:overworld run tp @s 8.5 70 8.5",
        );
        finish_trip(game, Dim::Overworld);
        assert_eq!(entity.tick_count(game), Some(29));
    });
}

#[test]
#[ignore = "daily"]
fn ordinary_end_exit_with_configured_respawn_keeps_player_tick_count() {
    with_server(ExecutionRole::DedicatedServer, |game| {
        let entity = game.entity(CONN).unwrap();
        game.lan.as_mut().unwrap().hub.update_guest(CONN, |guest| {
            guest.respawn_elsewhere =
                Some((Dim::Nether.id().to_owned(), [8, 70, 8], 0.0, 0.0, false));
        });
        game.as_guest(CONN, |game| {
            game.player.as_mut().unwrap().health.tick_count = 41;
        })
        .unwrap();
        prepare_end_exit(game, true);
        game.handle_server_portal(CONN);
        game.enter_level(Dim::Nether).unwrap();
        place_anchor(game, [8, 70, 8]);
        finish_trip(game, Dim::Nether);
        assert_eq!(entity.tick_count(game), Some(41));
    });
}

#[test]
#[ignore = "daily"]
fn credits_world_spawn_fallback_resets_player_tick_count() {
    with_server(ExecutionRole::DedicatedServer, |game| {
        let entity = game.entity(CONN).unwrap();
        game.as_guest(CONN, |game| {
            game.player.as_mut().unwrap().health.tick_count = 41;
        })
        .unwrap();
        prepare_end_exit(game, false);
        game.handle_server_portal(CONN);
        game.lan
            .as_mut()
            .unwrap()
            .play
            .get_mut(&CONN)
            .unwrap()
            .respawn_requested = true;
        game.handle_server_portal(CONN);
        finish_trip(game, Dim::Overworld);
        assert_eq!(entity.tick_count(game), Some(0));
    });
}

#[test]
#[ignore = "daily"]
fn credits_respawn_position_unchanged_resets_player_tick_count() {
    let mods = vec![ModEntry::new("keep-respawn", Box::new(KeepRespawn))].into();
    mod_platform::testing::with_mods(mods, || {
        with_server(ExecutionRole::DedicatedServer, |game| {
            let entity = game.entity(CONN).unwrap();
            game.as_guest(CONN, |game| {
                game.player.as_mut().unwrap().health.tick_count = 41;
            })
            .unwrap();
            prepare_end_exit(game, false);
            game.handle_server_portal(CONN);
            game.lan
                .as_mut()
                .unwrap()
                .play
                .get_mut(&CONN)
                .unwrap()
                .respawn_requested = true;
            game.handle_server_portal(CONN);
            finish_trip(game, Dim::Overworld);
            assert_eq!(entity.tick_count(game), Some(0));
        });
    });
}

#[test]
#[ignore = "daily"]
fn credits_missing_respawn_point_resets_player_tick_count() {
    with_server(ExecutionRole::DedicatedServer, |game| {
        let entity = game.entity(CONN).unwrap();
        game.lan.as_mut().unwrap().hub.update_guest(CONN, |guest| {
            guest.respawn_elsewhere =
                Some((Dim::Nether.id().to_owned(), [8, 70, 8], 0.0, 0.0, false));
        });
        game.as_guest(CONN, |game| {
            game.player.as_mut().unwrap().health.tick_count = 41;
        })
        .unwrap();
        prepare_end_exit(game, false);
        game.handle_server_portal(CONN);
        game.lan
            .as_mut()
            .unwrap()
            .play
            .get_mut(&CONN)
            .unwrap()
            .respawn_requested = true;
        game.handle_server_portal(CONN);
        finish_trip(game, Dim::Nether);
        assert_eq!(entity.tick_count(game), Some(0));
    });
}

#[test]
#[ignore = "daily"]
fn credits_respawn_position_redirect_resets_player_tick_count() {
    let mods = vec![ModEntry::new("move-respawn", Box::new(MoveRespawn))].into();
    mod_platform::testing::with_mods(mods, || {
        with_server(ExecutionRole::DedicatedServer, |game| {
            let entity = game.entity(CONN).unwrap();
            game.as_guest(CONN, |game| {
                game.player.as_mut().unwrap().health.tick_count = 41;
            })
            .unwrap();
            prepare_end_exit(game, false);
            game.handle_server_portal(CONN);
            game.lan
                .as_mut()
                .unwrap()
                .play
                .get_mut(&CONN)
                .unwrap()
                .respawn_requested = true;
            game.handle_server_portal(CONN);
            finish_trip(game, Dim::Overworld);
            assert_eq!(entity.tick_count(game), Some(0));
        });
    });
}

#[test]
#[ignore = "daily"]
fn credits_respawn_position_cross_dimension_resets_player_tick_count() {
    let mods = vec![ModEntry::new("cross-respawn", Box::new(CrossLevelRespawn))].into();
    mod_platform::testing::with_mods(mods, || {
        with_server(ExecutionRole::DedicatedServer, |game| {
            let entity = game.entity(CONN).unwrap();
            game.as_guest(CONN, |game| {
                game.player.as_mut().unwrap().health.tick_count = 41;
            })
            .unwrap();
            prepare_end_exit(game, false);
            game.handle_server_portal(CONN);
            game.lan
                .as_mut()
                .unwrap()
                .play
                .get_mut(&CONN)
                .unwrap()
                .respawn_requested = true;
            game.handle_server_portal(CONN);
            finish_trip(game, Dim::Overworld);
            assert_eq!(entity.tick_count(game), Some(0));
        });
    });
}

#[test]
#[ignore = "daily"]
fn credits_configured_respawn_keeps_permanent_player_attributes() {
    use sunrise_world::attributes::{Modifier, Operation};

    with_server(ExecutionRole::DedicatedServer, |game| {
        game.lan.as_mut().unwrap().hub.update_guest(CONN, |guest| {
            guest.respawn_elsewhere =
                Some((Dim::Nether.id().to_owned(), [8, 70, 8], 0.0, 0.0, false));
        });
        game.as_guest(CONN, |game| {
            let player = game.player.as_mut().unwrap();
            let key = player.attributes.key("minecraft:step_height").unwrap();
            let handle = player.attributes.get_instance(Some(key)).unwrap().unwrap();
            let modifier = Modifier::new("test:credits_step", 0.4, Operation::AddValue).unwrap();
            player
                .attributes
                .edit(&handle, |value, observer| {
                    value.add_permanent(modifier, observer)
                })
                .unwrap();
        })
        .unwrap();
        prepare_end_exit(game, false);
        game.handle_server_portal(CONN);
        game.lan
            .as_mut()
            .unwrap()
            .play
            .get_mut(&CONN)
            .unwrap()
            .respawn_requested = true;
        game.handle_server_portal(CONN);
        game.enter_level(Dim::Nether).unwrap();
        place_anchor(game, [8, 70, 8]);
        finish_trip(game, Dim::Nether);
        let step_height = game
            .as_guest(CONN, |game| {
                game.player
                    .as_mut()
                    .unwrap()
                    .attribute("minecraft:step_height")
                    .unwrap()
            })
            .unwrap();
        // PlayerMotion's default step height is 0.6; restore_all reapplies the
        // permanent +0.4 modifier, yielding the 1.0 covered by playerdata tests.
        assert!((step_height - 1.0).abs() < 1.0e-9);
    });
}

#[test]
#[ignore = "daily"]
fn death_respawn_starts_a_new_player_tick_count() {
    with_server(ExecutionRole::DedicatedServer, |game| {
        let entity = game.entity(CONN).unwrap();
        game.as_guest(CONN, |game| {
            game.player.as_mut().unwrap().health.tick_count = 41;
        })
        .unwrap();
        game.lan
            .as_mut()
            .unwrap()
            .hub
            .update_guest(CONN, |guest| guest.dead = true);
        game.guest_respawn(CONN);
        assert_eq!(entity.tick_count(game), Some(0));
    });
}

const OLD_COOLDOWN: &str = "minecraft:ender_pearl";
const NEW_COOLDOWN: &str = "minecraft:chorus_fruit";

type PositionSeen = Arc<Mutex<Vec<(Option<bool>, bool, bool)>>>;

struct EditDeathRespawn {
    redirect: bool,
    position_seen: PositionSeen,
}

#[mod_api::__subscriptions]
impl Hooks for EditDeathRespawn {
    fn player_respawn_position(
        &self,
        host: &mut dyn ServerHost,
        event: &mut PlayerRespawnPositionEvent,
    ) {
        let player = event.player;
        let alive = player.living().alive(host);
        let item_edited = player.set_item_cooldown(host, OLD_COOLDOWN, 18);
        let xp_edited = player.set_exp_cooldown(host, 8);
        self.position_seen
            .lock()
            .unwrap()
            .push((alive, item_edited, xp_edited));
        if self.redirect {
            *event.level = LevelDimension::Nether;
            *event.position = [8.5, 70.0, 8.5];
        }
    }

    fn player_respawned(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::PlayerRespawnedEvent<'_>,
    ) {
        if self.redirect
            && let Some(player) = host.player(event.player)
        {
            player.set_item_cooldown(host, NEW_COOLDOWN, 37);
        }
    }
}

#[test]
#[ignore = "daily"]
fn a_forced_respawn_does_not_copy_cooldowns_written_in_the_position_hook() {
    let position_seen = Arc::new(Mutex::new(Vec::new()));
    let mods = vec![ModEntry::new(
        "edit-dead-player",
        Box::new(EditDeathRespawn {
            redirect: false,
            position_seen: position_seen.clone(),
        }),
    )];
    mod_platform::testing::with_mods(mods.into(), || {
        with_server(ExecutionRole::DedicatedServer, |game| {
            let player = game.player(UUID).unwrap();
            game.lan.as_mut().unwrap().hub.update_guest(CONN, |guest| {
                guest.spawn = Some([5.5, 72.0, 5.5]);
                guest.dead = true;
                guest.health = 0.0;
            });
            game.guest_respawn(CONN);
            assert_eq!(*position_seen.lock().unwrap(), [(Some(false), true, true)]);
            assert_eq!(player.living().alive(game), Some(true));
            assert_eq!(player.has_item_cooldown(game, OLD_COOLDOWN), Some(false));
            assert_eq!(player.exp_cooldown(game), Some(0));
        });
    });
}

#[test]
#[ignore = "daily"]
fn an_unopenable_death_respawn_level_leaves_the_old_player_in_place() {
    use crate::game::native_player_death::tests::respawn_packets;
    use sunrise_protocol::packets::play::Clientbound as C;

    let position_seen = Arc::new(Mutex::new(Vec::new()));
    let mods = vec![ModEntry::new(
        "redirect-death-respawn",
        Box::new(EditDeathRespawn {
            redirect: true,
            position_seen: position_seen.clone(),
        }),
    )];
    mod_platform::testing::with_mods(mods.into(), || {
        with_server(ExecutionRole::DedicatedServer, |game| {
            let world = tempfile::tempdir().unwrap();
            game.world_root = world.path().to_path_buf();
            let record = world.path().join("data/sunrise/biome_regions.dat");
            std::fs::create_dir_all(record.parent().unwrap()).unwrap();
            std::fs::write(record, b"not a biome layout").unwrap();

            let (tx, rx) = std::sync::mpsc::channel();
            game.lan.as_mut().unwrap().hub.update_guest(CONN, |guest| {
                guest.sender = PlayerSender::new(tx, Arc::default());
                guest.spawn = Some([5.5, 72.0, 5.5]);
                guest.dead = true;
                guest.health = 0.0;
            });
            let player = game.player(UUID).unwrap();
            assert!(player.set_item_cooldown(game, "minecraft:shield", 23));
            game.guest_respawn(CONN);

            assert_eq!(*position_seen.lock().unwrap(), [(Some(false), true, true)]);
            assert_eq!(player.living().alive(game), Some(false));
            assert_eq!(
                player.has_item_cooldown(game, "minecraft:shield"),
                Some(true)
            );
            assert_eq!(player.has_item_cooldown(game, OLD_COOLDOWN), Some(true));
            assert_eq!(player.has_item_cooldown(game, NEW_COOLDOWN), Some(false));
            assert_eq!(player.exp_cooldown(game), Some(8));
            assert!(
                !respawn_packets(&rx)
                    .iter()
                    .any(|packet| matches!(packet, C::Respawn(_)))
            );
        });
    });
}

#[test]
#[ignore = "daily"]
fn a_deferred_cross_level_death_respawn_keeps_the_replacements_cooldown() {
    use crate::game::native_player_death::tests::{respawn_packets, with_respawn_client};
    use sunrise_protocol::packets::play::Clientbound as C;

    let position_seen = Arc::new(Mutex::new(Vec::new()));
    let mods = vec![ModEntry::new(
        "redirect-death-respawn",
        Box::new(EditDeathRespawn {
            redirect: true,
            position_seen: position_seen.clone(),
        }),
    )];
    let mut packets = Vec::new();
    mod_platform::testing::with_mods(mods.into(), || {
        with_server(ExecutionRole::DedicatedServer, |game| {
            // The destination chunks for the world spawn are not lit yet. The old body
            // has to remain available until the server finds the column and asks the mod.
            game.level.world_spawn = Some([96, 80, 96]);
            assert!(game.ui.rules.set_string("respawn_radius", "0"));
            let (tx, rx) = std::sync::mpsc::channel();
            game.lan.as_mut().unwrap().hub.update_guest(CONN, |guest| {
                guest.sender = PlayerSender::new(tx, Arc::default());
                guest.dead = true;
                guest.health = 0.0;
            });
            let player = game.player(UUID).unwrap();
            game.guest_respawn(CONN);
            game.tick_server_travel();
            assert!(game.on_server_trip(CONN));
            assert_eq!(player.living().alive(game), Some(false));
            assert!(position_seen.lock().unwrap().is_empty());
            assert!(respawn_packets(&rx).is_empty());

            finish_trip(game, Dim::Overworld);
            let guest = game.lan.as_ref().unwrap().hub.guest(CONN).unwrap();
            assert_eq!(guest.dimension, Dim::Nether.id());
            assert_eq!(guest.position, [8.5, 70.0, 8.5]);
            assert_eq!(*position_seen.lock().unwrap(), [(Some(false), true, true)]);
            assert_eq!(player.has_item_cooldown(game, OLD_COOLDOWN), Some(false));
            let cooldowns = player.item_cooldowns(game).unwrap();
            assert_eq!(cooldowns.len(), 1);
            assert_eq!(
                (cooldowns[0].group.as_str(), cooldowns[0].ticks_left),
                (NEW_COOLDOWN, 37)
            );
            packets = respawn_packets(&rx);
        });
    });

    let respawns: Vec<_> = packets
        .iter()
        .filter_map(|packet| match packet {
            C::Respawn(packet) => Some((packet.spawn.dimension.as_str(), packet.keep)),
            _ => None,
        })
        .collect();
    assert_eq!(respawns, [(Dim::Nether.id(), 0)]);
    with_respawn_client(|client| {
        client.set_screen(crate::game::Screen::Dead);
        for packet in packets {
            client.handle_packet(packet).unwrap();
        }
        assert!(!client.cooldowns.is_on_cooldown(OLD_COOLDOWN));
        assert_eq!(
            client
                .cooldowns
                .remaining()
                .map(|(group, ticks)| (group.as_ref(), ticks))
                .collect::<Vec<_>>(),
            [(NEW_COOLDOWN, 37)],
        );
    });
}

#[test]
#[ignore = "daily"]
fn an_end_gateway_jump_within_the_level_is_not_heard_on_any_server() {
    for role in SERVERS {
        heard_on(role, |game, heard| {
            game.build_registries();
            run_as_player(
                game,
                role,
                "setblock 3 64 5 minecraft:end_gateway{exit_portal:[I;40,64,40],ExactTeleport:0b}",
            );
            // Not exact: the player waits for the exit's chunks, then lands near it.
            game.start_gateway_trip(CONN, [3, 64, 5]);
            finish_trip(game, Dim::Overworld);
            let at = game.lan.as_ref().unwrap().hub.guest(CONN).unwrap().position;
            assert!(
                (at[0] - 40.5).hypot(at[2] - 40.5) < 16.0,
                "{role:?}: the jump landed at {at:?}"
            );
            assert!(heard.lock().unwrap().is_empty(), "{role:?}: {heard:?}");
        });
    }
}
