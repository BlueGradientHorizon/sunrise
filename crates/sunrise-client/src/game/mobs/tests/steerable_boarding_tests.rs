//! Witness for `Pig.mobInteract` and `Strider.mobInteract`: the real guest
//! Interact route, with only the world and connection constructed by the test.
use super::Game;
use crate::game::{
    Dim, execution_role::ExecutionRole, lan::LanSettings, native_entity_refs::tests,
};
use serde_json::json;
use std::sync::{Arc, atomic::AtomicI32, mpsc};
use sunrise_client::{
    chunk::Chunk,
    items::ItemStack,
    lan::{
        Joined, NetConfig, PlayerSender,
        guest::{Guest, InboxItem},
        hub::HubConfig,
    },
    mobs::Mob,
};
use sunrise_core::geometry::Vec3;
use sunrise_protocol::{
    buf::Reader,
    packets::{
        configuration::ClientInformation,
        play::{Interact, PlayerInput, Serverbound},
    },
};

const TEST: &str = "game::mounts::steerable_boarding_tests::interact_with_a_pig_or_strider";
const CHILD: &str = "SUNRISE_STEERABLE_BOARDING_CHILD";
const VEHICLE: u32 = 42;
const MOB_RIDER: u32 = 43;
const PLAYER: u32 = 44;
const CONN: u32 = 1;

fn scene(
    game: &mut Game<'_>,
    kind: &str,
    occupied: bool,
    saddled: bool,
    held: Option<&str>,
    secondary: bool,
) -> serde_json::Value {
    let at = Vec3::new(0.5, 100.0, 2.5);
    let mob_type = |id| game.res.mob_types().get(id).unwrap().clone();
    let mut vehicle = Mob::new(VEHICLE, mob_type(kind), at, 0.0);
    vehicle.set_saddle(saddled.then(|| ItemStack::new("minecraft:saddle", 1)));
    let mut passenger = Mob::new(MOB_RIDER, mob_type("minecraft:chicken"), at, 0.0);
    passenger.extra.mount = occupied.then_some(VEHICLE);
    game.mobs.clear();
    game.mobs.push(vehicle);
    if occupied {
        game.mobs.push(passenger);
    }

    let (sender, _packets) = mpsc::channel();
    let mut guest = Guest::new(
        Joined {
            conn: CONN,
            uuid: 1,
            name: "BoardingPlayer".into(),
            properties: Vec::new(),
            info: ClientInformation::new(2),
            brand: "fixture".into(),
            sender: PlayerSender::new(sender, Arc::default()),
            latency: Arc::new(AtomicI32::new(0)),
            address: "127.0.0.1:1".parse().unwrap(),
            expected: false,
        },
        PLAYER as i32,
        [0.5, 100.0, 0.5],
        2,
    );
    guest.dimension = game.dim.id().to_owned();
    guest.placed = true;
    guest.loaded = true;
    guest.load_timeout = 0;
    guest.inventory[guest.selected] = held.map(|item| ItemStack::new(item, 1));
    // PlayerInput and Interact agree about secondary use. Interact's handling
    // of stale PlayerInput is a different packet-parity problem.
    for packet in [
        Serverbound::PlayerInput(PlayerInput {
            flags: if secondary { 32 } else { 0 },
        }),
        Serverbound::Interact(Interact {
            id: VEHICLE as i32,
            hand: 0,
            location: [0.0, 0.45, 0.0],
            secondary,
        }),
    ] {
        let encoded = packet.encode();
        let mut reader = Reader::new(&encoded);
        let id = reader.varint().unwrap();
        guest.inbox.push_back(InboxItem::Packet(
            Serverbound::decode(id, reader.rest()).unwrap(),
            None,
        ));
    }
    let guests = &mut game.lan.as_mut().unwrap().hub.guests;
    guests.clear();
    guests.push(guest);
    game.reset_entity_residency();
    assert_eq!(
        game.has_seated_passengers(VEHICLE),
        occupied,
        "fixture seat"
    );
    game.level_pass = Default::default();
    game.tick_lan_packets();

    let guest = game.lan.as_ref().unwrap().hub.guest(CONN).unwrap();
    let mut seats = game
        .seated_passengers(VEHICLE)
        .into_iter()
        .map(|(_, id, player)| match (id, player) {
            (MOB_RIDER, false) => "mob",
            (PLAYER, true) => "player",
            _ => panic!("unexpected passenger {id}"),
        })
        .collect::<Vec<_>>();
    seats.sort_unstable();
    json!({"seats": seats, "riding": guest.vehicle == Some(VEHICLE),
           "held": guest.inventory[guest.selected].as_ref().map_or(0, |stack| stack.count),
           "saddled": game.mob_object(VEHICLE).unwrap().saddle_item().is_some()})
}

#[test]
#[ignore = "daily"]
fn interact_with_a_pig_or_strider() {
    if tests::run_fresh_cases(TEST, CHILD, &["boarding"]) {
        return;
    }
    tests::with_game_role(
        tests::fixture_root(),
        ExecutionRole::DedicatedServer,
        |game| {
            game.options.dedicated = Some(Vec::new());
            game.build_registries();
            game.lan_host(
                LanSettings {
                    port: 0,
                    guest_commands: false,
                    force_game_mode: false,
                },
                NetConfig {
                    listen: false,
                    announce: false,
                    lod_distance: 0,
                    ..NetConfig::lan(0, String::new(), 16)
                },
                HubConfig {
                    default_game_mode: 0,
                    guest_commands: false,
                    force_game_mode: false,
                    hardcore: false,
                    view_distance: 2,
                    simulation_distance: 2,
                    dimension: Dim::Overworld.id().to_owned(),
                    allow_flight: true,
                    seed: 0,
                    max_players: 16,
                    integrated: false,
                    levels: game.level_shapes,
                    level_seeds: game.levels.runtime.seeds(),
                },
            )
            .unwrap();
            game.streaming
                .insert_remote(&mut game.world, Chunk::empty(0, 0));
            game.next_entity = PLAYER + 1;
            let report = std::env::var_os("SUNRISE_STEERABLE_BOARDING_REPORT");
            let mut rows = serde_json::Map::new();
            for (kind, food) in [
                ("pig", "minecraft:carrot"),
                ("strider", "minecraft:warped_fungus"),
            ] {
                for (name, occupied, saddled, held, secondary, seats) in [
                    ("empty", false, true, None, false, vec!["player"]),
                    ("occupied", true, true, None, false, vec!["mob"]),
                    ("secondary", false, true, None, true, vec![]),
                    ("occupied_feed", true, true, Some(food), false, vec!["mob"]),
                    (
                        "occupied_saddle",
                        true,
                        false,
                        Some("minecraft:saddle"),
                        false,
                        vec!["mob"],
                    ),
                ] {
                    let label = format!("{kind}/{name}");
                    let observed = scene(
                        game,
                        &format!("minecraft:{kind}"),
                        occupied,
                        saddled,
                        held,
                        secondary,
                    );
                    if report.is_none() {
                        assert_eq!(
                            observed,
                            json!({"seats": seats, "riding": name == "empty",
                                                "held": 0, "saddled": true}),
                            "{label}"
                        );
                    }
                    rows.insert(label, observed);
                }
            }
            if let Some(path) = report {
                std::fs::write(path, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
            }
        },
    );
}
