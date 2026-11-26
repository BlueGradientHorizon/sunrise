use crate::game::server_side::lan::LanSettings;
use std::sync::{Arc, atomic::{AtomicI32, AtomicUsize}};
use sunrise_protocol::packets::configuration::ClientInformation;
use sunrise_sim::lan::{Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig};

pub fn with_guests(test: impl FnOnce(&mut crate::game::ServerGame<'_>)) {
    with_guests_at(super::native_entity_refs::tests::fixture_root(), test);
}

pub fn with_guests_at(
    root: std::path::PathBuf,
    test: impl FnOnce(&mut crate::game::ServerGame<'_>),
) {
    super::native_entity_refs::tests::with_game_at(root, |game| {
        let settings = LanSettings {
            port: 0,
            guest_commands: false,
            force_game_mode: false,
        };
        let net = NetConfig {
            listen: false,
            announce: false,
            lod_distance: 0,
            ..NetConfig::lan(0, String::new(), 2)
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
            max_players: 2,
            integrated: true,
            levels: game.server.level_shapes,
            level_seeds: game.server.levels.0.runtime.seeds(),
        };
        game.lan_host(settings, net, config).unwrap();
        for id in [12, 13] {
            // These guests bypass login's entity ID allocation; reserve their IDs before spawning.
            game.server.next_entity = game.server.next_entity.max(id);
            let (tx, _rx) = std::sync::mpsc::channel();
            let mut guest = Guest::new(
                Joined {
                    conn: id,
                    uuid: u128::from(id),
                    name: format!("guest{id}"),
                    properties: Vec::new(),
                    info: ClientInformation::new(8),
                    brand: String::new(),
                    sender: PlayerSender::new(tx, Arc::new(AtomicUsize::new(0))),
                    latency: Arc::new(AtomicI32::new(0)),
                    address: "127.0.0.1:1".parse().unwrap(),
                    expected: false,
                },
                id as i32,
                [3.0, 64.0, 5.0],
                10,
            );
            guest.placed = true;
            game.server.lan.as_mut().unwrap().hub.guests.push(guest);
        }
        test(game);
    });
}

/// Add a fixture menu to both the hub's protocol registry and the mod API's menu keys.
pub fn register_test_menu(
    game: &mut crate::game::ServerGame<'_>,
    id: &str,
) -> mod_api::MenuKey {
    use sunrise_protocol::registry::Ids;
    use sunrise_sim::lan::data::SyncData;

    let current = game.server.lan.as_ref().unwrap().hub.sync_data();
    let mut sync = SyncData::empty();
    sync.builtin = current.builtin.clone();
    sync.dynamic = current.dynamic.clone();
    sync.recipes = current.recipes.clone();
    sync.tags = current.tags.clone();
    let mut names = sync.builtin.get("minecraft:menu").unwrap().names.clone();
    names.push(id.to_owned());
    mod_api::__private::resolve_menus(names.iter().map(String::as_str)).unwrap();
    sync.builtin
        .registries
        .insert("minecraft:menu".into(), Ids::from_names(names));
    game.with_advancement_hub(|hub, owners| hub.reload_resources(Arc::new(sync), owners))
        .unwrap();
    mod_api::__private::menu_keys()
        .into_iter()
        .find(|key| mod_api::__private::menu_key_id(*key).as_deref() == Some(id))
        .unwrap()
}

