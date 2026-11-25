#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::loaded_arrow_shot_tests::natural;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::loaded_arrow_shot_tests::owner;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::loaded_arrow_shot_tests::start_events;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::loaded_arrow_shot_tests::stop_events;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::loaded_arrow_shot_tests::take_shots;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::loaded_arrow_shot_tests::with_game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::loaded_arrow_shot_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{
    Audio, Game, Options, Progress, Settings, load_resources,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    chunk::Chunk,
    mobs::Who,
    motion::PlayerMotion,
    projectiles::{Projectile, ProjectileKind},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::Vec3, position::ChunkPos, random::RandomSource,
};

#[test]
#[ignore = "daily"]
fn finish_hit_uses_registered_guest_context_and_restores_the_host() {
    use std::sync::{atomic::AtomicI32, mpsc};
    use sunrise_client::lan::{Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig};
    use sunrise_protocol::packets::configuration::ClientInformation;
    with_game(|game| {
        game.lan_host(
            crate::game::lan::LanSettings {
                port: 0,
                guest_commands: true,
                force_game_mode: false,
            },
            NetConfig {
                announce: false,
                ..NetConfig::loopback(0, "arrow-test".into(), 2)
            },
            HubConfig {
                default_game_mode: 1,
                guest_commands: true,
                force_game_mode: false,
                hardcore: false,
                view_distance: 2,
                simulation_distance: 2,
                dimension: "minecraft:overworld".into(),
                allow_flight: false,
                seed: 0,
                max_players: 2,
                integrated: false,
                levels: Default::default(),
                level_seeds: Default::default(),
            },
        )
        .unwrap();
        let (tx, _rx) = mpsc::channel();
        let mut guest = Guest::new(
            Joined {
                conn: 2,
                uuid: 114,
                name: "ArrowGuest".into(),
                properties: Vec::new(),
                info: ClientInformation::new(2),
                brand: "test".into(),
                sender: PlayerSender::new(tx, Arc::default()),
                latency: Arc::new(AtomicI32::new(0)),
                address: "127.0.0.1:25565".parse().unwrap(),
                expected: false,
            },
            2,
            [6.5, 200.0, 6.5],
            2,
        );
        guest.dimension = "minecraft:overworld".into();
        guest.placed = true;
        guest.loaded = true;
        guest.permission = 4;
        game.lan.as_mut().unwrap().hub.guests.push(guest);
        let host_uuid = game.acting_player_uuid();
        game.as_guest(2,|g| {
            assert_eq!(g.side.lan_acting,Some(2));
            assert_eq!(g.acting_player_uuid(),[0,0,0,114]);
            assert!(g.run_command("item replace entity @s weapon.mainhand with crossbow[charged_projectiles=[{id:\"minecraft:arrow\",count:1}]]"));
            let bow=g.held().unwrap().clone();assert!(g.use_crossbow(&bow));
        }).unwrap();
        assert_eq!(game.acting_player_uuid(), host_uuid);
        assert_eq!(game.side.lan_acting, None);
        let arrow = game.creatures.projectiles.pop().unwrap();
        assert_eq!(arrow.guest, Some(2));
        assert_eq!(arrow.owner, Some(Who::Player));
        assert_eq!(arrow.owner_uuid, Some([0, 0, 0, 114]));
        assert!(!arrow.arrow.as_ref().unwrap().has_been_shot);
        let at = [arrow.position.x, arrow.position.y, arrow.position.z];
        let id = arrow.id;
        start_events();
        game.finish_arrow_hit(arrow, super::EntityHit::Deflected);
        let first = take_shots();
        assert_eq!(game.acting_player_uuid(), host_uuid);
        assert_eq!(game.side.lan_acting, None);
        assert_eq!(game.lan.as_ref().unwrap().hub.guests[0].uuid, 114);
        assert!(
            game.creatures
                .projectiles
                .iter()
                .find(|p| p.id == id)
                .unwrap()
                .arrow
                .as_ref()
                .unwrap()
                .has_been_shot
        );
        game.tick_projectiles().unwrap();
        assert!(take_shots().is_empty());
        stop_events();
        assert_eq!(
            first,
            vec![("minecraft:projectile_shoot", at, Some(Who::Player))]
        );
    });
}
