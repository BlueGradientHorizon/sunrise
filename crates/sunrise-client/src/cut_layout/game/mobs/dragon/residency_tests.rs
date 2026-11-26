//! Production singleton installation/removal and id-allocation contracts.
use super::*;
use {sunrise_gameplay::game::native_entity_refs::tests::with_client_game, sunrise_gameplay::game::native_entity_refs::tests::with_game};

#[test]
#[ignore = "daily"]
fn singleton_dragon_spawn_install_refusal_and_kill() {
    with_game(|game| {
        game.dragon.discard();
        let before = game.side.next_entity;
        let id = game
            .spawn_dragon_spawned([0.0, 80.0, 0.0], SpawnSite::Plain)
            .unwrap();
        assert_eq!(id, before + 1);
        assert_eq!(game.side.next_entity, before + 9);
        for part in id + 1..=id + 8 {
            assert!(game.dragon_part(part).is_some());
        }
        assert!(
            game.spawn_dragon_spawned([1.0, 80.0, 0.0], SpawnSite::Plain)
                .is_none()
        );
        assert_eq!(game.side.next_entity, before + 9);
        let mut replacement = Dragon::new(Vec3::ZERO, 0.0, 0);
        replacement.id = 900;
        let refused = game.dragon.install(replacement).err().unwrap();
        assert_eq!(refused.id, 900);
        assert_eq!(game.dragon.as_ref().unwrap().id, id);
        game.kill_dragon(id + 1);
        assert_eq!(game.dragon.as_ref().unwrap().id, id);
        game.kill_dragon(id);
        assert!(game.dragon.is_none());
    });
}

#[test]
#[ignore = "daily"]
fn singleton_dragon_load_keeps_state_and_reserves_nine_ids() {
    with_game(|game| {
        game.dragon.discard();
        let mut saved = Dragon::new(Vec3::new(2.0, 90.0, 3.0), 25.0, 0);
        saved.health = 73.0;
        let uuid = [17, 0, 0, 3];
        let tag = saved.to_nbt(uuid, None, &Default::default());
        let before = game.side.next_entity;
        game.load_dragon(tag.clone(), Some(uuid));
        let dragon = game.dragon.as_ref().unwrap();
        assert_eq!(dragon.id, before + 1);
        assert_eq!(dragon.position, saved.position);
        assert_eq!(dragon.health, 73.0);
        assert!(dragon.join.is_joined());
        assert_eq!(game.dragon.locate(dragon.join), Some(0));
        assert_eq!(game.side.next_entity, before + 9);
        assert_eq!(game.persist.uuids.get(&dragon.id), Some(&uuid));
        game.load_dragon(tag, Some(uuid));
        assert_eq!(game.side.next_entity, before + 9);
        assert_eq!(game.dragon.as_ref().unwrap().id, before + 1);
        let chunks = game.entity_chunks(&[(0, 0)].into_iter().collect(), false);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].1.len(), 1);
        assert_eq!(
            chunks[0].1[0].get("Health"),
            Some(&Tag::Float(73.0f32.to_bits()))
        );
        assert_eq!(
            chunks[0].1[0].get("UUID"),
            Some(&Tag::IntArray(uuid.to_vec()))
        );
        let entity = game.entity(before + 1).unwrap();
        assert!(entity.discard(game));
        game.native_removals_flush();
        assert!(game.dragon.is_none());
    });
}

#[test]
#[ignore = "daily"]
fn singleton_fight_dragon_install_reserves_nine_ids() {
    with_game(|game| {
        game.dragon.discard();
        let before = game.side.next_entity;
        game.create_new_dragon();
        let dragon = game.dragon.as_ref().unwrap();
        assert_eq!(dragon.id, before + 1);
        assert_eq!(game.side.next_entity, before + 9);
        assert!(dragon.join.is_joined());
        assert_eq!(
            game.side.fight.dragon_uuid,
            game.persist.uuids.get(&dragon.id).copied()
        );
    });
}

fn install_server_dragon(game: &mut sunrise_gameplay::game::ServerGame<'_>, id: u32, at: [f64; 3]) {
    let mut dragon = Dragon::new(Vec3::new(at[0], at[1], at[2]), 0.0, 0);
    dragon.id = id;
    game.replace_resident_dragon(dragon);
}

#[test]
#[ignore = "daily"]
fn singleton_client_dragon_install_explicitly_replaces_network_owner() {
    with_client_game(|game| {
        game.dragon.discard();
        game.spawn_remote_dragon(420, [1.0, 80.0, 2.0], 45.0);
        assert_eq!(game.dragon.as_ref().unwrap().id, 420);
        game.spawn_remote_dragon(840, [3.0, 90.0, 4.0], 90.0);
        let dragon = game.dragon.as_ref().unwrap();
        assert_eq!(dragon.id, 840);
        assert_eq!(dragon.position, Vec3::new(3.0, 90.0, 4.0));
        game.remove_remote_dragon(420);
        assert_eq!(game.dragon.as_ref().unwrap().id, 840);
        game.remove_remote_dragon(840);
        assert!(game.dragon.is_none());
    });
}

#[test]
#[ignore = "daily"]
fn singleton_late_dragon_id_reserves_all_eight_parts_once() {
    with_game(|game| {
        game.dragon.discard();
        assert!(game.dragon.install(Dragon::new(Vec3::ZERO, 0.0, 0)).is_ok());
        let before = game.side.next_entity;
        game.assign_dragon_and_crystal_ids();
        assert_eq!(game.dragon.as_ref().unwrap().id, before + 1);
        assert_eq!(game.side.next_entity, before + 9);
        game.assign_dragon_and_crystal_ids();
        assert_eq!(game.side.next_entity, before + 9);
    });
}

#[test]
#[ignore = "daily"]
fn singleton_lifecycle_commands_unload_and_parked_owners() {
    crate::game::server_side::entity_server::taken_pass_tests::LifecycleFixture {
        seed: |game, entries| {
            game.discard_resident_dragon();
            for &(id, at) in entries {
                install_server_dragon(game, id, at);
            }
        },
        ids: |game| game.dragon.as_ref().map(|d| vec![d.id]).unwrap_or_default(),
        target: crate::game::commands::Target::Dragon,
        unloads_with_chunk: true,
        remote: None,
    }
    .run_singleton();
}

#[test]
#[ignore = "daily"]
fn singleton_death_fight_scan_and_resets_discard_owners() {
    with_game(|game| {
        install_server_dragon(game, 601, [1.0, 80.0, 1.0]);
        game.dragon.as_mut().unwrap().death_time = 199;
        game.tick_dragon_death();
        assert!(game.dragon.is_none(), "death completion");
        install_server_dragon(game, 602, [1.0, 80.0, 1.0]);
        game.scan_state();
        assert!(game.dragon.is_none(), "fight scan without exit portal");
        install_server_dragon(game, 603, [1.0, 80.0, 1.0]);
        game.clear_entities();
        assert!(game.dragon.is_none(), "world reset");
        install_server_dragon(game, 605, [1.0, 80.0, 1.0]);
        game.set_allow_commands(true);
        assert!(game.run_command("kill @e[type=minecraft:ender_dragon]"));
        assert!(game.dragon.is_none(), "command selector");
        install_server_dragon(game, 606, [1.0, 80.0, 1.0]);
        game.world.modified.clear();
        game.tick_dragon();
        assert!(
            game.world.modified.contains(&(0, 0)),
            "tick dirties the owner's chunk"
        );
    });
    with_client_game(|game| {
        game.spawn_remote_dragon(604, [1.0, 80.0, 1.0], 0.0);
        game.clear_remote_projectiles();
        assert!(game.dragon.is_none(), "remote reset");
    });
}

#[test]
#[ignore = "daily"]
fn singleton_join_rejection_discards_the_installed_dragon() {
    struct Veto;
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Veto {
        fn entity_join_level(
            &self,
            _: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::EntityJoinLevelEvent<'_>,
        ) -> mod_api::Flow {
            if event.kind == "minecraft:ender_dragon" {
                mod_api::Flow::Cancel
            } else {
                mod_api::Flow::Continue
            }
        }
    }
    mod_platform::testing::with_mods(
        vec![mod_api::ModEntry::new("dragon-veto", Box::new(Veto))].into(),
        || {
            with_game(|game| {
                assert!(
                    game.spawn_dragon_spawned([1.0, 80.0, 1.0], SpawnSite::Plain)
                        .is_none()
                );
                assert!(
                    game.dragon.is_none(),
                    "join rejection left its owner installed"
                );
            });
        },
    );
}

#[test]
#[ignore = "daily"]
fn singleton_live_owner_is_not_reported_as_departed() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Witness(Arc<AtomicUsize>);
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Witness {
        fn entity_leave_level(
            &self,
            _: &mut dyn mod_api::ServerHost,
            _: &mut mod_api::EntityLeaveLevelEvent<'_>,
        ) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let leaves = Arc::new(AtomicUsize::new(0));
    mod_platform::testing::with_mods(
        vec![mod_api::ModEntry::new(
            "dragon-leaves",
            Box::new(Witness(leaves.clone())),
        )]
        .into(),
        || {
            with_game(|game| {
                let id = game
                    .spawn_dragon_spawned([1.0, 80.0, 1.0], SpawnSite::Plain)
                    .unwrap();
                game.native_leaves_flush();
                assert_eq!(leaves.load(Ordering::Relaxed), 0);
                game.kill_dragon(id);
                game.native_leaves_flush();
                assert_eq!(leaves.load(Ordering::Relaxed), 1);
            });
        },
    );
}

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
