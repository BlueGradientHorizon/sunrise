

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod nested_spawn_tests {
    use super::*;
    use std::cell::Cell;
    const ID: u32 = 1_900_000_001;
    const AT: Vec3 = Vec3::new(3.5, 80.0, 3.5);
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::game) enum Point {
        Falling,
        Projectile,
        Trident,
        RemoteRocket,
        OwnRocket,
    }
    type Spawn = fn(&mut sunrise_client_gameplay::game::ClientGame<'_>, u32);
    thread_local! { static PENDING: Cell<Option<(Point, Spawn)>> = const { Cell::new(None) }; }
    pub(in sunrise_client_gameplay::game::client_side) fn after_lift(
        game: &mut sunrise_client_gameplay::game::ClientGame<'_>,
        point: Point,
    ) {
        if let Some((expected, spawn)) = PENDING.get()
            && point == expected
        {
            PENDING.set(None);
            spawn(game, ID);
        }
    }
    fn with_spawn(point: Point, spawn: Spawn, action: impl FnOnce()) {
        assert!(PENDING.replace(Some((point, spawn))).is_none());
        action();
        assert!(PENDING.take().is_none(), "requested lift was not exercised");
    }
    fn spawn(game: &mut sunrise_client_gameplay::game::ClientGame<'_>, kind: &str, data: i32, id: u32) {
        assert!(game.spawn_remote_projectile(
            kind,
            &sunrise_protocol::packets::play::AddEntity {
                id: id as i32,
                uuid: u128::from(id),
                kind: 0,
                position: [AT.x, AT.y, AT.z],
                velocity: [1.0, 0.0, 0.0],
                pitch: 0,
                yaw: 0,
                head_yaw: 0,
                data
            },
            (0.0, 0.0),
        ));
    }
    fn falling(game: &mut sunrise_client_gameplay::game::ClientGame<'_>, id: u32) {
        let sand = game
            .pack
            .assets
            .registry
            .default_state("minecraft:sand")
            .unwrap();
        spawn(game, "minecraft:falling_block", sand.0 as i32, id);
    }
    #[test]
    #[ignore = "daily"]
    fn client_falling_block_keeps_collision_bounce_through_drag() {
        sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
            use std::sync::Arc;
            let slime = game
                .pack
                .assets
                .registry
                .default_state("minecraft:slime_block")
                .unwrap();
            game.world
                .chunks
                .insert((0, 0), Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)));
            game.world.set(4, 63, 4, slime).unwrap();
            falling(game, 48);
            game.falling.update(0, |f| {
                f.position = Vec3::new(4.5, 64.25, 4.5);
                f.velocity = Vec3::new(0.0, -0.96, 0.0);
            });
            game.tick_remote_projectiles(|_, p| p);
            let f = &game.falling[0];
            assert_eq!(f.position.y.to_bits(), 64.0_f64.to_bits());
            assert_eq!(f.velocity.y.to_bits(), 0.7878808308504466_f64.to_bits());
            assert!(f.ground.on_ground);
        });
    }
    fn projectile(game: &mut sunrise_client_gameplay::game::ClientGame<'_>, id: u32) {
        spawn(game, "minecraft:shulker_bullet", 0, id);
    }
    fn trident(game: &mut sunrise_client_gameplay::game::ClientGame<'_>, id: u32) {
        spawn(game, "minecraft:trident", 0, id);
    }
    fn rocket(game: &mut sunrise_client_gameplay::game::ClientGame<'_>, id: u32) {
        spawn(game, "minecraft:firework_rocket", 0, id);
    }
    fn own_rocket(game: &mut sunrise_client_gameplay::game::ClientGame<'_>, id: u32) {
        rocket(game, id);
        let me = game.side.remote.as_ref().unwrap().entity_id as u32;
        game.rockets.for_each_mut(|r| {
            if r.id == id {
                r.attached_to = Some(me);
            }
        });
    }
    fn identities(
        game: &mut sunrise_client_gameplay::game::ClientGame<'_>,
        point: Point,
        remove: bool,
    ) -> Vec<(u32, sunrise_sim::join_seq::JoinSeq)> {
        macro_rules! rows {
            ($list:expr) => {{
                if remove {
                    let _ = $list.remove(1);
                }
                $list.iter().map(|e| (e.id, e.join)).collect()
            }};
        }
        match point {
            Point::Falling => rows!(game.falling),
            Point::Projectile => rows!(game.creatures.projectiles),
            Point::Trident => rows!(game.tridents),
            Point::RemoteRocket | Point::OwnRocket => rows!(game.rockets),
        }
    }
    struct Case {
        point: Point,
        spawn: Spawn,
        position: fn(&sunrise_client_gameplay::game::ClientGame<'_>) -> Vec3,
    }
    #[test]
    #[ignore = "daily"]
    fn remote_lift_restores_each_nested_spawn_without_ticking_it_early() {
        sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            game.block_ticks
                .add_empty_chunk(sunrise_core::position::ChunkPos::new(0, 0))
                .unwrap();
            for case in [
                Case {
                    point: Point::Falling,
                    spawn: falling,
                    position: |g| g.falling.iter().find(|e| e.id == ID).unwrap().position,
                },
                Case {
                    point: Point::Projectile,
                    spawn: projectile,
                    position: |g| {
                        g.creatures
                            .projectiles
                            .iter()
                            .find(|e| e.id == ID)
                            .unwrap()
                            .position
                    },
                },
                Case {
                    point: Point::Trident,
                    spawn: trident,
                    position: |g| g.tridents.iter().find(|e| e.id == ID).unwrap().position,
                },
                Case {
                    point: Point::RemoteRocket,
                    spawn: rocket,
                    position: |g| g.rockets.iter().find(|e| e.id == ID).unwrap().position,
                },
            ] {
                game.clear_remote_projectiles();
                for id in [71, 72, 73] {
                    (case.spawn)(game, id);
                }
                let mut expected = identities(game, case.point, true);
                with_spawn(case.point, case.spawn, || {
                    game.tick_remote_projectiles(|_, at| at)
                });
                assert_eq!(
                    (case.position)(game),
                    AT,
                    "{:?} ticked during its spawn pass",
                    case.point
                );
                let actual = identities(game, case.point, false);
                expected.push(*actual.iter().find(|e| e.0 == ID).unwrap());
                assert_eq!(
                    actual, expected,
                    "{:?} whole-list identity/order",
                    case.point
                );
                game.tick_remote_projectiles(|_, at| at);
                assert!(
                    (case.position)(game).x > AT.x,
                    "{:?} missed the next pass",
                    case.point
                );
            }
        });
    }
    #[test]
    #[ignore = "daily"]
    fn own_rocket_lift_keeps_a_new_attachment_until_the_next_pass() {
        sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
            let (client, _server) = sunrise_protocol::connection::Pipe::pair();
            game.connect_local(client, "NestedRocket").unwrap();
            game.side.remote.as_mut().unwrap().entity_id = 99;
            game.player = Some(
                sunrise_sim::motion::PlayerMotion::new(game.motion, Vec3::new(20.0, 80.0, 20.0))
                    .unwrap(),
            );
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            game.block_ticks
                .add_empty_chunk(sunrise_core::position::ChunkPos::new(0, 0))
                .unwrap();
            for id in [7, 8, 9] {
                own_rocket(game, id);
            }
            let mut expected = identities(game, Point::OwnRocket, true);
            with_spawn(Point::OwnRocket, own_rocket, || game.tick_own_rockets());
            assert_eq!(
                game.rockets.iter().find(|r| r.id == ID).unwrap().position,
                AT
            );
            let actual = identities(game, Point::OwnRocket, false);
            expected.push(*actual.iter().find(|e| e.0 == ID).unwrap());
            assert_eq!(actual, expected);
            game.tick_own_rockets();
            assert_ne!(
                game.rockets.iter().find(|r| r.id == ID).unwrap().position,
                AT
            );
        });
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod entity_tick_tests {
    use std::sync::{Arc, Mutex};

    /// The tick hooks a listener heard: whether on a client, the phase, entities and kinds.
    type Event = (bool, &'static str, Vec<u32>, Vec<String>, Vec<[f64; 3]>);
    type Heard = Arc<Mutex<Vec<Event>>>;

    /// A server's `AddEntity` of an arrow, through the packet path that tracks it: a listener's
    /// handle for a client entity resolves only for one the server told this client of.
    fn add_arrow(game: &mut sunrise_client_gameplay::game::ClientGame<'_>, id: i32, position: [f64; 3]) {
        let kind = game
            .side
            .remote
            .as_ref()
            .unwrap()
            .builtin
            .get("minecraft:entity_type")
            .unwrap()
            .id("minecraft:arrow")
            .unwrap();
        game.handle_packet(sunrise_protocol::packets::play::Clientbound::AddEntity(
            sunrise_protocol::packets::play::AddEntity {
                id,
                uuid: id as u128,
                kind,
                position,
                velocity: [1.0, 0.0, 0.0],
                pitch: 0,
                yaw: 0,
                head_yaw: 0,
                data: 0,
            },
        ))
        .unwrap();
    }

    struct Listener(Heard);

    impl Listener {
        fn record(
            &self,
            host: &mod_api::HostSide<'_>,
            phase: &'static str,
            entities: &[u32],
            kinds: &[&str],
        ) {
            let client = matches!(host, mod_api::HostSide::Client(_));
            let kinds = kinds.iter().map(|kind| (*kind).to_owned()).collect();
            let positions = match host {
                mod_api::HostSide::Client(host) => entities
                    .iter()
                    .map(|&id| host.entity(id).unwrap().position(*host).unwrap())
                    .collect(),
                _ => Vec::new(),
            };
            self.0
                .lock()
                .unwrap()
                .push((client, phase, entities.to_vec(), kinds, positions));
        }
    }

    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Listener {
        fn entity_tick_listened(
            &self,
            _: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickListenedEvent,
        ) {
            event.listened = true;
        }
        fn entity_tick_pre(
            &self,
            host: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickPreEvent<'_>,
        ) {
            self.record(&host, "pre", event.entities, event.kinds);
        }
        fn entity_tick_post(
            &self,
            host: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickPostEvent<'_>,
        ) {
            self.record(&host, "post", event.entities, event.kinds);
        }
    }

    /// The same listener, naming arrows to `entity_tick_each`.
    struct Named(Heard);

    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Named {
        fn entity_tick_listened(
            &self,
            _: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickListenedEvent,
        ) {
            event.listened = true;
        }
        fn entity_tick_pre(
            &self,
            host: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickPreEvent<'_>,
        ) {
            Listener(self.0.clone()).record(&host, "pre", event.entities, event.kinds);
        }
        fn entity_tick_post(
            &self,
            host: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickPostEvent<'_>,
        ) {
            Listener(self.0.clone()).record(&host, "post", event.entities, event.kinds);
        }
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            r.entity_tick_each("minecraft:arrow");
        }
    }

    /// Two remote arrows of a type a mod named: on the client too each is told alone, its Post
    /// right after its own flight and before the next arrow's Pre.
    #[test]
    #[ignore = "daily"]
    fn named_remote_arrows_are_told_one_by_one() {
        let heard: Heard = Arc::default();
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "client-entity-ticks",
            Box::new(Named(heard.clone())),
        )]
        .into();
        let mut positions = Vec::new();
        mod_platform::testing::with_mods(mods, || {
            sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
                game.world.chunks.insert(
                    (0, 0),
                    std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
                );
                game.block_ticks
                    .add_empty_chunk(sunrise_core::position::ChunkPos::new(0, 0))
                    .unwrap();
                let _server = sunrise_client_gameplay::game::client_side::native_client_session::tests::logged_client(game);
                for (id, z) in [(71, 3.5), (72, 5.5)] {
                    add_arrow(game, id, [3.5, 80.0, z]);
                }
                game.tick_remote_projectiles(|_, at| at);
                positions.extend(
                    game.creatures
                        .projectiles
                        .iter()
                        .map(|p| [p.position.x, p.position.y, p.position.z]),
                );
            });
        });
        let heard = heard.lock().unwrap();
        let phases: Vec<_> = heard
            .iter()
            .map(|(client, phase, entities, _, _)| (*client, *phase, entities.clone()))
            .collect();
        assert_eq!(
            phases,
            [
                (true, "pre", vec![71]),
                (true, "post", vec![71]),
                (true, "pre", vec![72]),
                (true, "post", vec![72]),
            ]
        );
        assert_eq!(heard[0].4, [[3.5, 80.0, 3.5]]);
        assert_eq!(heard[1].4, [positions[0]]);
        assert_eq!(heard[2].4, [[3.5, 80.0, 5.5]]);
        assert_eq!(heard[3].4, [positions[1]]);
        assert!(positions.iter().all(|p| p[0] > 3.5));
    }

    /// A client's listener hears a remote arrow's own tick there, between the server's
    /// updates: its Pre before the arrow flies on, its Post after, on the client side.
    #[test]
    #[ignore = "daily"]
    fn a_client_listener_hears_a_remote_arrows_tick() {
        let heard: Heard = Arc::default();
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "client-entity-ticks",
            Box::new(Listener(heard.clone())),
        )]
        .into();
        let mut after = [0.0; 3];
        mod_platform::testing::with_mods(mods, || {
            sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
                game.world.chunks.insert(
                    (0, 0),
                    std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
                );
                game.block_ticks
                    .add_empty_chunk(sunrise_core::position::ChunkPos::new(0, 0))
                    .unwrap();
                let _server = sunrise_client_gameplay::game::client_side::native_client_session::tests::logged_client(game);
                add_arrow(game, 71, [3.5, 80.0, 3.5]);
                game.tick_remote_projectiles(|_, at| at);
                let arrow = game.creatures.projectiles.iter().find(|p| p.id == 71);
                let p = arrow.unwrap().position;
                after = [p.x, p.y, p.z];
                assert!(after[0] > 3.5, "the arrow flew on");
            });
        });
        let arrow = || (vec![71], vec!["minecraft:arrow".to_owned()]);
        let heard = heard.lock().unwrap();
        assert_eq!(
            *heard,
            [
                (true, "pre", arrow().0, arrow().1, vec![[3.5, 80.0, 3.5]]),
                (true, "post", arrow().0, arrow().1, vec![after])
            ]
        );
    }
}