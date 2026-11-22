use super::*;

#[test]
fn the_creative_sync_covers_inventory_menu_slots_5_to_45_once_each() {
    // CreativeInventoryListener sends every InventoryMenu slot but the crafting ones: the
    // armour (5..=8), the inventory (9..=44) and the off hand (45).
    let stack = |item: &str| Some(sunrise_client::items::ItemStack::new(item, 1));
    let inventory: Vec<_> = (0..36)
        .map(|i| stack(&format!("minecraft:item_{i}")))
        .collect();
    let offhand = stack("minecraft:shield");
    let armor = [
        stack("minecraft:diamond_helmet"),
        stack("minecraft:diamond_chestplate"),
        stack("minecraft:diamond_leggings"),
        stack("minecraft:diamond_boots"),
    ];
    let stacks = crate::game::remote::creative_sync_stacks(&inventory, &offhand, &armor);
    assert_eq!(stacks.len(), crate::game::remote::CREATIVE_SYNCED);
    let mut slots: Vec<i16> = (0..stacks.len())
        .map(crate::game::remote::creative_sync_slot)
        .collect();
    // Each entry goes to the menu slot that holds that stack.
    for (i, &slot) in slots.iter().enumerate() {
        let game_slot = Game::inventory_screen_slot("inventory", i32::from(slot)).unwrap();
        let expected = match game_slot {
            0..36 => &inventory[game_slot],
            504 => &offhand,
            500..504 => &armor[game_slot - 500],
            _ => panic!("entry {i} went to game slot {game_slot}"),
        };
        assert_eq!(&stacks[i], expected, "entry {i}, menu slot {slot}");
    }
    slots.sort();
    assert_eq!(slots, (5..=45).collect::<Vec<i16>>());
}

#[test]
fn a_server_slot_update_leaves_an_unsent_creative_click_to_be_sent() {
    let stack = |item: &str| Some(sunrise_client::items::ItemStack::new(item, 1));
    let none = || vec![None; crate::game::remote::CREATIVE_SYNCED];
    // Synced: nothing. A click puts a helmet on (entry 37, not sent yet), then the server
    // sets hotbar slot 0 (a pickup): only that entry counts as synced.
    let mut synced = none();
    let mut before = none();
    before[37] = stack("minecraft:diamond_helmet");
    let mut now = before.clone();
    now[0] = stack("minecraft:stick");
    crate::game::remote::mark_synced(&mut synced, &before, now.clone());
    assert_eq!(synced[0], now[0]);
    assert_eq!(
        synced[37], None,
        "the helmet still differs, so the next tick sends it"
    );
}

mod give_stack {
    use super::*;
    use crate::game::api_values::test_snapshot;
    use crate::game::native_entity_refs::tests::with_client_game;
    use crate::game::native_hotbar_slot::hotbar_slot_tests::{logged_in, sent};
    use crate::game::remote::creative_sync_slot;
    use mod_api::CreativeGiveError;
    use sunrise_core::nbt::{Compound, JavaString, Tag};
    use sunrise_protocol::packets::play::Serverbound as S;

    fn api(item: &str, count: u32, components: &[(&str, Tag)]) -> mod_api::ItemStack {
        let mut stack =
            mod_api::ItemStack::new(mod_api::__private::item_key(item).unwrap(), count).unwrap();
        if !components.is_empty() {
            stack
                .set_components(
                    components
                        .iter()
                        .map(|(id, tag)| (JavaString::from(*id), tag.clone()))
                        .collect(),
                )
                .unwrap();
        }
        stack
    }

    fn named(item: &str, count: u32, name: &str) -> mod_api::ItemStack {
        api(
            item,
            count,
            &[("minecraft:custom_name", Tag::String(name.into()))],
        )
    }

    /// The `SetCreativeModeSlot` packets among `packets`: wire slot and stack.
    fn creative_slots(packets: &[S]) -> Vec<(i16, Option<sunrise_protocol::item::Stack>)> {
        packets
            .iter()
            .filter_map(|packet| match packet {
                S::SetCreativeModeSlot(p) => Some((p.slot, p.item.clone())),
                _ => None,
            })
            .collect()
    }

    /// What the window sends for inventory slot `i` (36: the off hand).
    fn wire(game: &Game<'_>, i: usize) -> Option<sunrise_protocol::item::Stack> {
        let stack = if i == 36 {
            game.offhand.clone()
        } else {
            game.inventory[i].clone()
        };
        game.to_network(&stack)
    }

    #[test]
    #[ignore = "daily"]
    fn a_refused_give_changes_and_sends_nothing() {
        with_client_game(|game| {
            let stone = api("minecraft:stone", 1, &[]);
            assert_eq!(game.give_stack(&stone), Err(CreativeGiveError::Unavailable));
            let mut reader = logged_in(game);
            let inventory = game.inventory.clone();
            assert_eq!(game.give_stack(&stone), Err(CreativeGiveError::NotCreative));
            game.set_mode_id(1);
            assert_eq!(
                game.give_stack(&api("minecraft:stone", 65, &[])),
                Err(CreativeGiveError::Stack(
                    mod_api::StackWriteError::CountExceeded { maximum: 64 }
                ))
            );
            assert_eq!(
                game.give_stack(&api(
                    "minecraft:iron_sword",
                    1,
                    &[("minecraft:max_stack_size", Tag::Int(16))]
                )),
                Err(CreativeGiveError::Stack(
                    mod_api::StackWriteError::StackableDamageable
                ))
            );
            game.options.execution_role =
                crate::game::execution_role::ExecutionRole::IntegratedServer;
            assert_eq!(game.give_stack(&stone), Err(CreativeGiveError::Unavailable));
            game.options.execution_role = crate::game::execution_role::ExecutionRole::Client;
            assert_eq!(game.inventory, inventory);
            assert!(creative_slots(&sent(&mut reader)).is_empty());
        });
    }

    /// Components reach the slot and the server unchanged, and the slot goes out at once both
    /// with nothing open and while a server menu is open (whose end-of-tick sync skips it).
    #[test]
    #[ignore = "daily"]
    fn a_give_keeps_components_and_sends_its_slots_at_once() {
        with_client_game(|game| {
            let mut reader = logged_in(game);
            game.set_mode_id(1);
            game.selected = 0;
            let mut sword = api(
                "minecraft:iron_sword",
                1,
                &[("minecraft:enchantments", {
                    let mut levels = Compound::new();
                    levels.insert("minecraft:sharpness".into(), Tag::Int(3));
                    Tag::Compound(levels)
                })],
            );
            sword.set_damage(11);
            let potion = api(
                "minecraft:potion",
                1,
                &[("minecraft:potion_contents", {
                    let mut contents = Compound::new();
                    contents.insert("potion".into(), Tag::String("minecraft:swiftness".into()));
                    Tag::Compound(contents)
                })],
            );
            let capped = api(
                "minecraft:stone",
                16,
                &[("minecraft:max_stack_size", Tag::Int(16))],
            );
            for (slot, stack) in [&sword, &potion, &capped].into_iter().enumerate() {
                assert_eq!(game.give_stack(stack), Ok(None));
                assert_eq!(
                    game.inventory[slot].as_ref().map(test_snapshot).as_ref(),
                    Some(stack)
                );
                assert_eq!(
                    creative_slots(&sent(&mut reader)),
                    [(creative_sync_slot(slot), wire(game, slot))]
                );
            }
            // A full capped stack takes no more: the next goes to an empty slot.
            assert_eq!(
                game.give_stack(&api(
                    "minecraft:stone",
                    4,
                    &[("minecraft:max_stack_size", Tag::Int(16),)]
                )),
                Ok(None)
            );
            assert_eq!(game.inventory[2].as_ref().map(|s| s.count), Some(16));
            assert_eq!(game.inventory[3].as_ref().map(|s| s.count), Some(4));
            assert_eq!(
                creative_slots(&sent(&mut reader)),
                [(creative_sync_slot(3), wire(game, 3))]
            );

            game.remote
                .as_mut()
                .unwrap()
                .menu
                .set(Some((1, "minecraft:generic_9x3".into())));
            let name = named("minecraft:paper", 2, "letter");
            assert_eq!(game.give_stack(&name), Ok(None));
            assert_eq!(game.inventory[4].as_ref().map(test_snapshot), Some(name));
            assert_eq!(
                creative_slots(&sent(&mut reader)),
                [(creative_sync_slot(4), wire(game, 4))]
            );
            // The slot counts as synced: the tick's own sync sends it again neither way.
            game.remote_send_state(0);
            game.remote.as_mut().unwrap().menu.set(None);
            game.remote_send_state(0);
            assert!(creative_slots(&sent(&mut reader)).is_empty());
        });
    }

    /// `Inventory.add`'s order: the selected slot, the off hand (wire slot 45), then the rest,
    /// never onto a stack with other components; what does not fit comes back.
    #[test]
    #[ignore = "daily"]
    fn a_give_stacks_in_inventory_order_and_returns_what_did_not_fit() {
        with_client_game(|game| {
            let mut reader = logged_in(game);
            game.set_mode_id(1);
            game.selected = 4;
            let plain = |count| {
                Some(sunrise_client::items::ItemStack::new(
                    "minecraft:stone",
                    count,
                ))
            };
            game.inventory[4] = plain(60);
            game.offhand = plain(60);
            game.inventory[0] = plain(63);
            game.inventory[9] = plain(10);
            let mut renamed = sunrise_client::items::ItemStack::new("minecraft:stone", 1);
            renamed.data_mut().custom_name = Some(Tag::String("other".into()));
            game.inventory[1] = Some(renamed.clone());
            game.remote_send_state(0);
            sent(&mut reader);

            assert_eq!(game.give_stack(&api("minecraft:stone", 64, &[])), Ok(None));
            assert_eq!(game.inventory[4].as_ref().map(|s| s.count), Some(64));
            assert_eq!(game.offhand.as_ref().map(|s| s.count), Some(64));
            assert_eq!(game.inventory[0].as_ref().map(|s| s.count), Some(64));
            assert_eq!(game.inventory[9].as_ref().map(|s| s.count), Some(64));
            assert_eq!(game.inventory[1], Some(renamed));
            assert_eq!(game.inventory[2].as_ref().map(|s| s.count), Some(1));
            let mut slots = creative_slots(&sent(&mut reader));
            slots.sort_by_key(|(slot, _)| *slot);
            let mut expected: Vec<_> = [0, 2, 4, 9, 36]
                .into_iter()
                .map(|i| (creative_sync_slot(i), wire(game, i)))
                .collect();
            expected.sort_by_key(|(slot, _)| *slot);
            assert_eq!(slots, expected);
            assert!(slots.iter().any(|(slot, _)| *slot == 45));

            let dirt = plain(64).map(|s| sunrise_client::items::ItemStack {
                item: "minecraft:dirt".into(),
                ..s
            });
            game.inventory[..36].fill(dirt.clone());
            game.offhand = dirt;
            game.remote_send_state(0);
            sent(&mut reader);
            let inventory = game.inventory.clone();
            let gift = named("minecraft:stone", 5, "left over");
            assert_eq!(game.give_stack(&gift), Ok(Some(gift)));
            assert_eq!(game.inventory, inventory);
            assert!(creative_slots(&sent(&mut reader)).is_empty());
        });
    }
}
