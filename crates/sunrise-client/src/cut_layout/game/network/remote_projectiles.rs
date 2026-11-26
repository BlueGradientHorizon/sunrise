#![cfg(test)]
#[cfg(feature = "client")]
#[cfg(test)]
pub(crate) mod entity_tick_tests {
    use std::sync::{Arc, Mutex};

    /// The tick hooks a listener heard: whether on a client, the phase, entities and kinds.
    type Event = (bool, &'static str, Vec<u32>, Vec<String>, Vec<[f64; 3]>);
    type Heard = Arc<Mutex<Vec<Event>>>;

    /// A server's `AddEntity` of an arrow, through the packet path that tracks it: a listener's
    /// handle for a client entity resolves only for one the server told this client of.
    fn add_arrow(
        game: &mut sunrise_client_gameplay::game::ClientGame<'_>,
        id: i32,
        position: [f64; 3],
    ) {
        let kind = game
            .client
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
            crate::game::native_entity_refs::tests::with_client_game(|game| {
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
            crate::game::native_entity_refs::tests::with_client_game(|game| {
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
use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
