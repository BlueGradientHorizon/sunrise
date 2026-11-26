#![cfg(test)]
#[cfg(test)]
mod tests {
    use super::super::native_entity_refs::tests::with_game;
    
    use mod_api::{CustomSpawners, SpawnerEntry, SpawnerKey, SpawnerKind};
    use std::sync::{Arc, Mutex};

    type Log = Arc<Mutex<Vec<String>>>;

    /// A mod that takes the cat spawner out and adds spawners `<name>:swarm`,
    /// and logs every `custom_spawner_tick` and `modify_custom_spawners` it hears.
    struct Spawns {
        name: &'static str,
        log: Log,
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Spawns {
        fn modify_custom_spawners(
            &self,
            _: &dyn mod_api::Host,
            event: &mut mod_api::ModifyCustomSpawnersEvent<'_>,
        ) {
            self.log
                .lock()
                .unwrap()
                .push(format!("{} modify", self.name));
            event
                .spawners
                .remove(&SpawnerEntry::Builtin(SpawnerKind::Cat));
            event
                .spawners
                .add(SpawnerKey::new(&format!("{}:swarm", self.name)).unwrap());
        }
        fn custom_spawner_tick(
            &self,
            _: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::CustomSpawnerTickEvent<'_>,
        ) {
            self.log
                .lock()
                .unwrap()
                .push(format!("{} sees {}", self.name, event.key.as_str()));
        }
    }

    /// With no listener the list is vanilla's and every spawner ticks: the cat
    /// spawner counts its clock down. With two mods, `modify_custom_spawners`
    /// fires once per level, the cat spawner never ticks, and the mods' spawners
    /// tick in list order (the order they were added), each asked of every mod
    /// in dependency order.
    #[test]
    #[ignore = "daily"]
    fn custom_spawners_tick_in_list_order_and_a_removed_one_never_ticks() {
        with_game(|game| {
            game.server.cat.next_tick = 5;
            game.tick_custom_spawners();
            assert_eq!(
                game.server.cat.next_tick, 4,
                "no listener: vanilla's cat spawner ticks"
            );
            assert_eq!(
                game.creatures.custom_spawners[&game.dim],
                CustomSpawners::vanilla()
            );
        });
        let log = Log::default();
        let mods = ["amod", "bmod"]
            .map(|name| {
                mod_api::ModEntry::new(
                    name,
                    Box::new(Spawns {
                        name,
                        log: log.clone(),
                    }),
                )
            })
            .into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                game.server.cat.next_tick = 5;
                game.tick_custom_spawners();
                game.tick_custom_spawners();
                assert_eq!(
                    game.server.cat.next_tick, 5,
                    "the removed cat spawner never ticks"
                );
            });
        });
        let log = log.lock().unwrap();
        assert_eq!(
            log[..2],
            ["amod modify", "bmod modify"],
            "one modify event per level, in dependency order"
        );
        assert_eq!(log.iter().filter(|l| l.ends_with("modify")).count(), 2);
        let ticks: Vec<&str> = log[2..].iter().map(String::as_str).collect();
        let once = [
            "amod sees amod:swarm",
            "bmod sees amod:swarm",
            "amod sees bmod:swarm",
            "bmod sees bmod:swarm",
        ];
        assert_eq!(ticks, [once, once].concat());
    }
}
