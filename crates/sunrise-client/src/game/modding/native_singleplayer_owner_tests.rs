//! `Players::singleplayer_profile` and `PlayerRef::is_singleplayer_owner` (#4636) on a real
//! hub: the integrated server's launch profile whether or not its owner is online, matched to
//! players by name ignoring case (`IntegratedServer.isSingleplayerOwner`), and nothing on a
//! dedicated server (`DedicatedServer.isSingleplayerOwner`).

use super::execution_role::ExecutionRole;
use super::lan::LanSettings;
use mod_api::{PlayerRef, prelude::*};
use std::sync::{
    Arc,
    atomic::{AtomicI32, AtomicUsize},
};
use sunrise_client::lan::{Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig};
use sunrise_protocol::packets::configuration::ClientInformation;

const OWNER: u128 = 1;

/// A server on `role` whose login keeps `singleplayer` as the integrated server's launch
/// profile, with no player online yet.
fn with_server(
    role: ExecutionRole,
    singleplayer: Option<(&str, u128)>,
    test: impl FnOnce(&mut super::Game<'_>),
) {
    let root = super::native_entity_refs::tests::fixture_root();
    super::native_entity_refs::tests::with_game_role(root, role, |game| {
        let settings = LanSettings {
            port: 0,
            guest_commands: false,
            force_game_mode: false,
        };
        let net = NetConfig {
            listen: false,
            announce: false,
            lod_distance: 0,
            singleplayer_profile: singleplayer.map(|(name, uuid)| (name.to_owned(), uuid)),
            ..NetConfig::lan(0, String::new(), 4)
        };
        let config = HubConfig {
            default_game_mode: 0,
            guest_commands: false,
            force_game_mode: false,
            hardcore: false,
            view_distance: 2,
            simulation_distance: 2,
            dimension: game.dim.id().to_owned(),
            allow_flight: true,
            seed: 0,
            max_players: 4,
            integrated: singleplayer.is_some(),
            levels: game.level_shapes,
            level_seeds: game.levels.runtime.seeds(),
        };
        game.lan_host(settings, net, config).unwrap();
        test(game);
    });
}

/// A player joins as `name` with `uuid` and returns their handle.
fn join(game: &mut super::Game<'_>, conn: u32, uuid: u128, name: &str) -> PlayerRef {
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut guest = Guest::new(
        Joined {
            conn,
            uuid,
            name: name.to_owned(),
            properties: Vec::new(),
            info: ClientInformation::new(8),
            brand: String::new(),
            sender: PlayerSender::new(tx, Arc::new(AtomicUsize::new(0))),
            latency: Arc::new(AtomicI32::new(0)),
            address: "127.0.0.1:1".parse().unwrap(),
            expected: false,
        },
        conn as i32,
        [3.0, 64.0, 5.0],
        10,
    );
    guest.placed = true;
    game.lan.as_mut().unwrap().hub.guests.push(guest);
    let players = game.players().server_players();
    *players.iter().find(|player| player.uuid() == uuid).unwrap()
}

fn leave(game: &mut super::Game<'_>, conn: u32) {
    game.lan
        .as_mut()
        .unwrap()
        .hub
        .guests
        .retain(|g| g.conn != conn);
}

/// The launch profile's name and UUID, as a mod reads it.
fn profile(game: &super::Game<'_>) -> Option<(String, u128)> {
    let profile = game.players().singleplayer_profile()?;
    Some((profile.name.to_owned(), profile.uuid))
}

#[test]
#[ignore = "daily"]
fn the_integrated_owner_is_the_launch_profile_matched_by_name() {
    with_server(
        ExecutionRole::IntegratedServer,
        Some(("Owner", OWNER)),
        |game| {
            // Commands off: the owner is no operator (`IntegratedPlayerList`).
            let hub = &mut game.lan.as_mut().unwrap().hub;
            hub.ops.insert(OWNER, 0);
            hub.singleplayer_owner = Some(OWNER);
            let launched = Some(("Owner".to_owned(), OWNER));
            assert_eq!(profile(game), launched, "before the owner joins");

            let guest = join(game, 12, 12, "guest12");
            game.lan.as_mut().unwrap().hub.ops.insert(12, 4);
            assert!(
                !guest.is_singleplayer_owner(game),
                "a guest who joined first, op 4"
            );

            let owner = join(game, 13, OWNER, "Owner");
            assert!(owner.is_singleplayer_owner(game), "the owner at op 0");
            assert!(!guest.is_singleplayer_owner(game));

            leave(game, 13);
            assert_eq!(profile(game), launched, "after the owner leaves");
            assert!(!owner.is_singleplayer_owner(game), "a stale handle");

            let returning = join(game, 14, OWNER, "Owner");
            assert!(returning.is_singleplayer_owner(game), "the returning owner");
            leave(game, 14);

            // The name decides, ignoring case; the UUID plays no part.
            let namesake = join(game, 15, 99, "oWNER");
            assert!(namesake.is_singleplayer_owner(game));
            assert_eq!(
                profile(game),
                launched,
                "the profile keeps the owner's UUID"
            );
        },
    );
}

#[test]
#[ignore = "daily"]
fn a_dedicated_server_has_no_singleplayer_owner() {
    with_server(ExecutionRole::DedicatedServer, None, |game| {
        let op = join(game, 12, OWNER, "Owner");
        game.lan.as_mut().unwrap().hub.ops.insert(OWNER, 4);
        assert_eq!(profile(game), None);
        assert!(!op.is_singleplayer_owner(game));
    });
}
