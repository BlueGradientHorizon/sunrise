// Client reads of vanilla state for mods (#4677), on a window joined to a server through
// memory and fed the server's packets as it would receive them.
use crate::game::{Game, Screen, native_entity_refs::tests::with_client_game};
use mod_api::ClientHost;
use sunrise_client::chunk::Chunk;
use sunrise_protocol::{
    connection::{ConnReader, ConnWriter},
    packets::play::{self, Clientbound as C},
};

/// The window's own player.
const PLAYER: u32 = 7;

fn spawn() -> play::SpawnInfo {
    play::SpawnInfo {
        dimension_type: 0,
        dimension: "minecraft:overworld".into(),
        seed: 0,
        game_type: 0,
        previous_game_type: None,
        debug: false,
        flat: true,
        last_death: None,
        portal_cooldown: 0,
        sea_level: 63,
    }
}

/// The server's teleport that makes the local player, on a stone floor, with the level loaded.
fn stand(game: &mut Game<'_>, teleport: i32) {
    game.streaming
        .insert_remote(&mut game.world, Chunk::empty(0, 0));
    let stone = game
        .pack
        .assets
        .registry
        .default_state("minecraft:stone")
        .unwrap();
    for x in 0..16 {
        for z in 0..16 {
            game.world.set(x, 63, z, stone).unwrap();
        }
    }
    game.handle_packet(C::PlayerPosition(play::PlayerPosition {
        teleport,
        position: [0.5, 64.0, 0.5],
        delta: [0.0; 3],
        yaw: 0.0,
        pitch: 0.0,
        relatives: 0,
    }))
    .unwrap();
    let remote = game.remote.as_mut().unwrap();
    remote.loaded_sent = true;
    remote.level_load.finish();
    game.screen = Screen::Playing;
}

/// Joins as entity [`PLAYER`] in the overworld.
fn join(game: &mut Game<'_>) -> (ConnReader, ConnWriter) {
    let connection = super::connect_menu_test(game);
    game.handle_packet(C::Login(play::Login {
        player_id: PLAYER as i32,
        hardcore: false,
        levels: vec!["minecraft:overworld".into()],
        max_players: 20,
        chunk_radius: 2,
        simulation_distance: 2,
        reduced_debug_info: false,
        show_death_screen: true,
        limited_crafting: false,
        spawn: spawn(),
        online_mode: false,
        enforces_secure_chat: false,
    }))
    .unwrap();
    stand(game, 1);
    connection
}

/// The server adds entity `id` of `kind` beside the player.
fn add(game: &mut Game<'_>, id: u32, kind: &str) {
    let kind = game
        .remote
        .as_ref()
        .unwrap()
        .builtin
        .get("minecraft:entity_type")
        .unwrap()
        .id(kind)
        .unwrap();
    game.handle_packet(C::AddEntity(play::AddEntity {
        id: id as i32,
        uuid: 0x4677_0000 + u128::from(id),
        kind,
        position: [2.5, 64.0, 2.5],
        velocity: [0.0; 3],
        pitch: 0,
        yaw: 0,
        head_yaw: 0,
        data: 0,
    }))
    .unwrap();
}

/// `EntityRef::tick_count` of entity `id`, read through a client host.
fn tick_count(game: &Game<'_>, id: u32) -> Option<i32> {
    game.entity(id)?.tick_count(game as &dyn ClientHost)
}

/// `Entity.tickCount` on a client is its copy's own counter: `ClientLevel.tickNonPassenger`'s
/// `commonTick` counts each client tick from the entity's arrival, and the new `LocalPlayer`
/// that `handleRespawn` makes starts again at 0.
#[test]
#[ignore = "daily"]
fn a_client_counts_its_copies_ticks_and_a_respawned_player_starts_at_zero() {
    crate::game::in_own_process(
        "game::remote::tests::client_vanilla_reads_tests::a_client_counts_its_copies_ticks_and_a_respawned_player_starts_at_zero",
        || {
            with_client_game(|game| {
                let _connection = join(game);
                add(game, 20, "minecraft:pig");
                add(game, 21, "minecraft:minecart");
                let counts = |game: &Game<'_>| [PLAYER, 20, 21].map(|id| tick_count(game, id));
                assert_eq!(counts(game), [Some(0); 3]);
                for _ in 0..3 {
                    game.tick_remote().unwrap();
                }
                assert_eq!(counts(game), [Some(3); 3]);

                game.handle_packet(C::Respawn(play::Respawn {
                    spawn: spawn(),
                    keep: 0,
                }))
                .unwrap();
                stand(game, 2);
                assert_eq!(counts(game), [Some(0), Some(3), Some(3)]);
                game.tick_remote().unwrap();
                assert_eq!(counts(game), [Some(1), Some(4), Some(4)]);
            });
        },
    );
}

/// The two facts `Hud.nextContextualInfoState` reads, as a client mod reads them:
/// `LocalPlayer.jumpableVehicle` (a saddled mount that jumps, not a pig) and
/// `ClientWaypointManager.hasWaypoints` (any waypoint the server tracks for this client).
#[test]
#[ignore = "daily"]
fn the_contextual_bar_facts_read_the_jumpable_mount_and_the_tracked_waypoints() {
    use sunrise_client::locator_bar::ContextualBar;
    use sunrise_protocol::packets::play::{TrackedWaypoint, WaypointId, WaypointTarget};
    crate::game::in_own_process(
        "game::remote::tests::client_vanilla_reads_tests::the_contextual_bar_facts_read_the_jumpable_mount_and_the_tracked_waypoints",
        || {
            with_client_game(|game| {
                let _connection = join(game);
                let facts = |game: &Game<'_>| {
                    let host: &dyn ClientHost = game;
                    (host.jumpable_vehicle(), host.has_waypoints())
                };
                assert_eq!(facts(game), (None, false));
                let ride = |game: &mut Game<'_>, vehicle: u32, passengers: Vec<i32>| {
                    game.handle_packet(C::SetPassengers(play::SetPassengers {
                        vehicle: vehicle as i32,
                        passengers,
                    }))
                    .unwrap();
                };
                for (id, kind) in [(30, "minecraft:horse"), (31, "minecraft:pig")] {
                    add(game, id, kind);
                    let mount = game.mobs.iter_mut().find(|m| m.id == id).unwrap();
                    mount.extra.set_saddled(true);
                }
                ride(game, 30, vec![PLAYER as i32]);
                let horse = game.entity(30);
                assert!(horse.is_some());
                assert_eq!(facts(game), (horse, false));
                assert_eq!(game.contextual_bar(), ContextualBar::JumpableVehicle);
                // A saddle only steers a pig.
                ride(game, 30, Vec::new());
                ride(game, 31, vec![PLAYER as i32]);
                assert_eq!(facts(game), (None, false));

                let waypoint = |operation| {
                    C::TrackedWaypoint(TrackedWaypoint {
                        operation,
                        id: WaypointId::Uuid(0x4677),
                        style: "minecraft:default".into(),
                        color: None,
                        target: WaypointTarget::Block([40, 64, 40]),
                    })
                };
                game.handle_packet(waypoint(0)).unwrap();
                assert_eq!(facts(game), (None, true));
                assert_eq!(game.contextual_bar(), ContextualBar::Locator);
                game.handle_packet(waypoint(1)).unwrap();
                assert_eq!(facts(game), (None, false));
            });
        },
    );
}
