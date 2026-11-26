

#![cfg(test)]
#[cfg(test)]
pub(crate) mod sweep_tests {
    use sunrise_gameplay_common::game::native_hooks::entity_inside::sweep_tests::InsidePart;
    use sunrise_gameplay_common::game::native_hooks::entity_inside::sweep_tests::install_inside_mod;
    use sunrise_gameplay::game::native_hooks::entity_inside::sweep_tests::item_at_blocks;

    /// A killing write stops the item's flush: the walk still reaches both blocks,
    /// the writes after the destruction never apply.
    #[test]
    #[ignore = "daily"]
    fn a_killing_write_stops_the_items_flush() {
        sunrise_gameplay_common::game::in_own_process(
            "game::server_side::native_hooks_server::entity_inside::sweep_tests::a_killing_write_stops_the_items_flush",
            || {
                let part = InsidePart {
                    hurt_after: Some((0, 0.5)),
                    ..InsidePart::default()
                };
                let mods = install_inside_mod("inside-item-kill", &part);
                mod_platform::testing::with_mods(mods, || {
                    sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
                        let _ = sunrise_sim::block_contact::HOOKS.set(sunrise_gameplay_common::game::native_hooks::BLOCK_CONTACT_HOOKS);
                        let mut item = item_at_blocks(game);
                        use sunrise_items::items::ItemStack;
                        let diamonds = ItemStack::new("minecraft:diamond", 3);
                        item.stack = ItemStack::new("minecraft:shulker_box", 1);
                        item.stack.data_mut().container = vec![(0, diamonds.clone())];
                        item.health = 1;
                        let before = item.velocity.x;
                        sunrise_sim::block_contact::in_level(
                            sunrise_sim::parallel::TickThread::mint(),
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
                        game.side.tickets.0.update(None);
                        game.side.ticked_entities_through = u32::MAX;
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
                            sunrise_sim::block_contact::in_level(
                                sunrise_sim::parallel::TickThread::mint(),
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