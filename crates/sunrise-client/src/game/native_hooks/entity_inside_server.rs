#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::entity_inside::*;

#[cfg(test)]
pub(in crate::game) mod sweep_tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::entity_inside::sweep_tests::InsideMod;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::entity_inside::sweep_tests::InsidePart;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::entity_inside::sweep_tests::arrow_shot;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::entity_inside::sweep_tests::install_inside_mod;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::entity_inside::sweep_tests::item_at_blocks;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::entity_inside::sweep_tests::sweep_game_player;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::entity_inside::sweep_tests::sweep_game_player_runs;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::entity_inside::sweep_tests::*;

    /// A killing write stops the item's flush: the walk still reaches both blocks,
    /// the writes after the destruction never apply.
    #[test]
    #[ignore = "daily"]
    fn a_killing_write_stops_the_items_flush() {
        crate::game::in_own_process(
            "game::server_side::native_hooks_server::entity_inside::sweep_tests::a_killing_write_stops_the_items_flush",
            || {
                let part = InsidePart {
                    hurt_after: Some((0, 0.5)),
                    ..InsidePart::default()
                };
                let mods = install_inside_mod("inside-item-kill", &part);
                mod_platform::testing::with_mods(mods, || {
                    crate::game::native_entity_refs::tests::with_game(|game| {
                        let _ =
                            sunrise_client::block_contact::HOOKS.set(super::BLOCK_CONTACT_HOOKS);
                        let mut item = item_at_blocks(game);
                        use sunrise_client::items::ItemStack;
                        let diamonds = ItemStack::new("minecraft:diamond", 3);
                        item.stack = ItemStack::new("minecraft:shulker_box", 1);
                        item.stack.data_mut().container = vec![(0, diamonds.clone())];
                        item.health = 1;
                        let before = item.velocity.x;
                        sunrise_client::block_contact::in_level(
                            sunrise_client::parallel::TickThread::mint(),
                            Some("minecraft:overworld"),
                            || {
                                game.sweep_item_inside(&mut item);
                            },
                        );
                        assert_eq!(part.contacts.lock().unwrap().len(), 2);
                        assert_eq!(item.health, 0, "subtract 0.5 before truncating health");
                        assert_eq!(game.dropped.len(), 1);
                        assert_eq!(game.dropped[0].stack, diamonds);
                        assert_eq!(game.dropped[0].position, item.position);
                        assert_eq!(
                            item.velocity.x.to_bits(),
                            before.to_bits(),
                            "the writes after the destruction never apply"
                        );

                        // Run the real batch on a merge tick. The victim touches a
                        // non-solid block, its neighbour is close enough to merge
                        // but doesn't touch that block.
                        let grass = game
                            .pack
                            .assets
                            .registry
                            .default_state("minecraft:short_grass")
                            .unwrap();
                        game.world.set(1, 104, 0, grass).unwrap();
                        game.tickets.0.update(None);
                        game.server.ticked_entities_through = u32::MAX;
                        for (health, count) in [(5, 3), (1, 2)] {
                            part.contacts.lock().unwrap().clear();
                            game.dropped.clear();
                            for (x, count, ticks, health) in
                                [(1.25, 1, 39, health), (0.75, 2, 0, 5)]
                            {
                                game.spawn_item(
                                    ItemStack::new("minecraft:stone", count),
                                    [x, 104.5, 0.5],
                                    [0.0; 3],
                                    20,
                                );
                                let item = game.dropped.last_mut().unwrap();
                                item.no_gravity = true;
                                item.health = health;
                                item.ticks = ticks;
                            }
                            let neighbour = game.dropped[1].id;
                            sunrise_client::block_contact::in_level(
                                sunrise_client::parallel::TickThread::mint(),
                                Some("minecraft:overworld"),
                                || game.tick_items(),
                            );
                            assert_eq!(*part.contacts.lock().unwrap(), [[1, 104, 0]]);
                            assert_eq!(game.dropped.len(), 1);
                            assert_eq!(game.dropped[0].id, neighbour);
                            assert_eq!(game.dropped[0].stack.count, count);
                        }
                    });
                });
            },
        );
    }
}
