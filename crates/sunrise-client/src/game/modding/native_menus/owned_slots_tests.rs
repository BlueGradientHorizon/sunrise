use super::tests::{MOD_MENU, mod_menu_guest, sent};
use crate::game::{
    Game,
    api_values::{self, TryIntoApi},
    native_entity_refs::tests::with_game,
};
use mod_api::{
    Flow, Hand, Hooks, HostTransaction, ItemPlace, ItemResource, MenuClickedEvent, MenuClosedEvent,
    MenuItemsRefusal, MenuKey, MenuPart, MenuRef, MenuReplace, MenuSlotBackRefusal,
    MenuSlotBacking, MenuSlotPlace, MenuStillValidEvent, ModEntry, Mods, Registrar, ServerHost,
};
use std::sync::mpsc;
use sunrise_client::{items::ItemStack, lan::Out};
use sunrise_protocol::packets::play::{
    Clientbound as C, ContainerClick, ContainerClose, Serverbound as S, SetCursorItem,
};

struct Part;
#[mod_api::sunrise_part]
impl MenuPart for Part {
    fn screen(
        &self,
        _: &dyn mod_api::Host,
        _: &mut mod_api::ScreenEvent<'_>,
    ) -> Option<mod_api::Screen> {
        Some(mod_api::Screen {
            slots: vec![Some(mod_api::ScreenSlot::default()); 2],
            ..Default::default()
        })
    }

    fn menu_still_valid(&self, _: &dyn mod_api::LoopContext, event: &mut MenuStillValidEvent<'_>) {
        if let Some(item) = event.opening_item {
            assert_eq!(item.hand, Hand::Off);
            event.valid = item.stack_matches;
        }
    }
}

struct Listener {
    return_contents: bool,
}

#[mod_api::__subscriptions]
impl Hooks for Listener {
    fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        r.menu_part(MOD_MENU, Part);
    }

    fn menu_clicked(&self, host: &mut dyn ServerHost, event: &mut MenuClickedEvent) -> Flow {
        if event.slot != 1 {
            return Flow::Continue;
        }
        let menu = host.menu(event.uuid, event.window).unwrap();
        let Some(cursor) = menu.carried(host).flatten() else {
            return Flow::Continue;
        };
        host.transaction(|host, tx| {
            ItemPlace::MenuSlot { menu, slot: 1 }.insert(host, tx, &ItemResource::of(&cursor), 1);
            Ok::<_, ()>(())
        })
        .unwrap();
        Flow::Cancel
    }

    fn menu_closed(&self, host: &mut dyn ServerHost, event: &mut MenuClosedEvent<'_>) {
        if !self.return_contents {
            return;
        }
        assert_eq!(event.opening_hand, Some(Hand::Off));
        assert!(host.menu(event.uuid, event.window).is_none());
        let player = host.player(event.uuid).unwrap();
        host.transaction(|host, _| {
            for stack in event.owned_slots.iter().flatten() {
                assert!(player.give_stack(host, stack.clone()).unwrap().is_none());
            }
            Ok::<_, ()>(())
        })
        .unwrap();
    }
}

fn with_owned_menu(
    return_contents: bool,
    test: impl FnOnce(&mut Game<'_>, mpsc::Receiver<Out>, MenuKey),
) {
    let mods: Mods = vec![ModEntry::new(
        "owned",
        Box::new(Listener { return_contents }),
    )]
    .into();
    mod_platform::testing::with_mods(mods.clone(), || {
        api_values::install_parts(&mods).unwrap();
        let _callbacks = sunrise_client::native_client_hooks::scoped_menu_validity_hooks(
            api_values::menu_still_valid,
            |menu| mod_api::__private::menu_listens(mod_api::HookId::MenuStillValid, menu),
        );
        sunrise_client::containers::with_mod_menu_types(
            [(
                MOD_MENU.into(),
                "owned".into(),
                2,
                0,
                Vec::new(),
                Vec::new(),
                None,
                None,
                false,
                Some(vec![1, 4]),
            )],
            || {
                with_game(|game| {
                    let (rx, key) = mod_menu_guest(game);
                    game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                        g.sender.menu_extra_rev = sunrise_protocol::menu_extra::LIVE_DATA_REVISION;
                        g.offhand = Some(ItemStack::new("minecraft:paper", 2));
                    });
                    test(game, rx, key);
                });
            },
        );
    });
}

fn open(game: &mut Game<'_>, key: MenuKey) -> (i32, MenuRef) {
    let player = game.player(123).unwrap();
    // Open from the acting player's item action, when the hand lives in the game,
    // not in the guest's record.
    let window = game
        .as_guest(1, |game| {
            player.open_mod_menu(
                game,
                mod_api::ModMenuOpen {
                    menu: key,
                    title: mod_api::Text::literal("Owned slots"),
                    extra: Vec::new(),
                    data: Vec::new(),
                    pos: None,
                    opening_hand: Some(Hand::Off),
                    replace: MenuReplace::CloseFirst,
                },
            )
        })
        .unwrap()
        .unwrap()
        .unwrap();
    (window, game.player_container_menu(player).unwrap())
}

fn click(game: &mut Game<'_>, container: i32, slot: i16) {
    game.lan_guest_packet(
        1,
        S::ContainerClick(ContainerClick {
            container,
            state: 0,
            slot,
            button: 0,
            input: 0,
            changed: Vec::new(),
            carried: None,
        }),
    );
    game.run_native_menu_events();
}

#[test]
#[ignore = "daily"]
fn an_item_opened_menu_has_two_empty_owned_slots_with_individual_caps() {
    with_owned_menu(false, |game, rx, key| {
        let (window, menu) = open(game, key);
        let packets = sent(&rx);
        assert!(packets.iter().any(|p| matches!(p,
            C::ContainerSetContent(content) if content.container == window
                && content.items == vec![None, None]
        )));
        assert_eq!(menu.slot_count(game), Some(2));
        let player = game.player(123).unwrap();
        assert_eq!(
            player.back_mod_menu_slots(
                game,
                window,
                vec![MenuSlotPlace::HeldItem {
                    hand: Hand::Off,
                    slot: 0
                }],
            ),
            Err(MenuSlotBackRefusal::UnsupportedMenu),
        );
        let stick =
            mod_api::ItemStack::of(mod_api::__private::item_key("minecraft:stick").unwrap());
        let resource = ItemResource::of(&stick);
        for (slot, cap) in [1, 4].into_iter().enumerate() {
            assert_eq!(menu.slot(game, slot), Some(None));
            assert_eq!(
                menu.slot_backing(game, slot),
                Some(MenuSlotBacking::Owned(slot))
            );
            assert_eq!(menu.slot_holds(game, slot, &stick), Some(cap));
            let host: &mut dyn ServerHost = game;
            host.transaction(|host, tx| {
                let place = ItemPlace::MenuSlot { menu, slot };
                assert_eq!(place.insert(host, tx, &resource, 8), cap);
                assert_eq!(place.stack(host).unwrap().count(), cap);
                Ok::<_, ()>(())
            })
            .unwrap();
        }
        let host: &mut dyn ServerHost = game;
        assert_eq!(
            host.menu_set_items(123, window, vec![]),
            Err(MenuItemsRefusal::OwnedSlots)
        );
        assert_eq!(menu.slot(host, 0), Some(Some(stick)));
    });
}

#[test]
#[ignore = "daily"]
fn owned_click_prediction_applies_caps_and_the_server_corrects_an_override() {
    use crate::game::execution_role::ExecutionRole;
    use sunrise_client::menu::Input;
    use sunrise_protocol::{connection::Pipe, item::Hashed};
    with_owned_menu(false, |game, rx, key| {
        let (window, menu) = open(game, key);
        let opening = sent(&rx);
        let state = opening
            .iter()
            .find_map(|p| match p {
                C::ContainerSetContent(p) if p.container == window => Some(p.state),
                _ => None,
            })
            .unwrap();
        let stack = |count| Some(ItemStack::new("minecraft:stick", count));
        let hub = &mut game.lan.as_mut().unwrap().hub;
        let builtin = hub.sync_data().builtin.clone();
        let network = [1, 3, 4, 6, 7].map(|count| hub.stack(&stack(count)).unwrap());
        let [one, three, four, six, seven] = &network;
        let prediction = |slot, placed, left| {
            S::ContainerClick(ContainerClick {
                container: window,
                state,
                slot,
                button: 0,
                input: 0,
                changed: vec![(slot, Some(Hashed::of(placed)))],
                carried: Some(Hashed::of(left)),
            })
        };
        let set_server_cursor = |game: &mut Game<'_>| {
            game.lan
                .as_mut()
                .unwrap()
                .hub
                .guests
                .update(0, |g| g.carried = stack(7));
        };
        set_server_cursor(game);
        game.lan_guest_packet(1, prediction(0, one, six));
        assert_eq!(
            menu.slot(game, 0),
            Some(Some(stack(1).unwrap().try_into_api().unwrap()))
        );
        assert_eq!(
            menu.carried(game),
            Some(Some(stack(6).unwrap().try_into_api().unwrap()))
        );
        assert!(
            sent(&rx).is_empty(),
            "the server agreed with the ordinary click"
        );

        game.lan_guest_packet(
            1,
            S::ContainerClick(ContainerClick {
                container: window,
                state,
                slot: 0,
                button: 0,
                input: sunrise_client::lan::menu::input::QUICK_MOVE,
                changed: Vec::new(),
                carried: Some(Hashed::of(six)),
            }),
        );
        assert_eq!(
            menu.slot(game, 0),
            Some(Some(stack(1).unwrap().try_into_api().unwrap())),
            "shift-click must not move an item to an omitted player inventory"
        );
        assert!(sent(&rx).is_empty());

        set_server_cursor(game);
        game.lan_guest_packet(1, prediction(1, four, three));
        assert_eq!(
            menu.slot(game, 1),
            Some(Some(stack(1).unwrap().try_into_api().unwrap()))
        );
        assert_eq!(
            menu.carried(game),
            Some(Some(stack(7).unwrap().try_into_api().unwrap()))
        );
        let corrections = sent(&rx);
        assert!(corrections.iter().any(|p| matches!(p,
            C::ContainerSetSlot(p) if p.container == window && p.slot == 1 && p.item.as_ref() == Some(one)
        )));
        assert!(corrections.iter().any(|p| matches!(p,
            C::SetCursorItem(p) if p.item.as_ref() == Some(seven)
        )));

        // Run the real client click path and apply those authoritative packets. Both sides
        // know the declared caps; only the server executes the part's override.
        game.lan = None;
        game.options.execution_role = ExecutionRole::Client;
        let (client, _peer) = Pipe::pair();
        game.connect_local(client, "Owned slots").unwrap();
        game.remote.as_mut().unwrap().builtin = std::sync::Arc::new(builtin);
        for packet in opening {
            game.handle_packet(packet).unwrap();
        }
        assert_eq!(game.slots(), [300, 301]);
        let cursor = || {
            C::SetCursorItem(SetCursorItem {
                item: Some(seven.clone()),
            })
        };
        game.handle_packet(cursor()).unwrap();
        assert!(game.menu_ops(&[(Some(300), 0, Input::Pickup)]));
        assert_eq!(*game.slot_mut(300), stack(1));
        assert_eq!(game.carried, stack(6));
        assert!(game.menu_ops(&[(Some(300), 0, Input::QuickMove)]));
        assert_eq!(*game.slot_mut(300), stack(1));

        game.handle_packet(cursor()).unwrap();
        assert!(game.menu_ops(&[(Some(301), 0, Input::Pickup)]));
        assert_eq!(*game.slot_mut(301), stack(4));
        assert_eq!(game.carried, stack(3));
        for packet in corrections {
            game.handle_packet(packet).unwrap();
        }
        assert_eq!(*game.slot_mut(300), stack(1));
        assert_eq!(*game.slot_mut(301), stack(1));
        assert_eq!(game.carried, stack(7));
    });
}

#[test]
#[ignore = "daily"]
fn closing_discards_owned_slots_unless_the_listener_returns_their_contents() {
    for return_contents in [false, true] {
        with_owned_menu(return_contents, |game, _rx, key| {
            let (window, menu) = open(game, key);
            let stick =
                mod_api::ItemStack::of(mod_api::__private::item_key("minecraft:stick").unwrap());
            let host: &mut dyn ServerHost = game;
            host.transaction(|host, tx| {
                assert_eq!(
                    ItemPlace::MenuSlot { menu, slot: 1 }.insert(
                        host,
                        tx,
                        &ItemResource::of(&stick),
                        3
                    ),
                    3
                );
                Ok::<_, ()>(())
            })
            .unwrap();
            game.lan_guest_packet(1, S::ContainerClose(ContainerClose { container: window }));
            game.run_native_menu_events();
            assert_eq!(menu.slot(game, 1), None);
            let guest = &game.lan.as_ref().unwrap().hub.guests[0];
            assert_eq!(
                guest
                    .inventory
                    .iter()
                    .flatten()
                    .cloned()
                    .collect::<Vec<_>>(),
                if return_contents {
                    vec![ItemStack::new("minecraft:stick", 3)]
                } else {
                    vec![]
                }
            );
            assert!(game.dropped.is_empty());
            let (_, reopened) = open(game, key);
            assert_eq!(reopened.slot(game, 1), Some(None));
        });
    }
}

#[test]
#[ignore = "daily"]
fn abort_after_replacement_restores_only_committed_contents_to_the_old_close() {
    with_owned_menu(true, |game, _rx, key| {
        let (_, old) = open(game, key);
        let player = game.player(123).unwrap();
        let stick =
            mod_api::ItemStack::of(mod_api::__private::item_key("minecraft:stick").unwrap());
        let resource = ItemResource::of(&stick);
        let old_slot = ItemPlace::MenuSlot { menu: old, slot: 1 };
        let host: &mut dyn ServerHost = game;
        host.transaction(|host, tx| {
            assert_eq!(old_slot.insert(host, tx, &resource, 2), 2);
            Ok::<_, ()>(())
        })
        .unwrap();

        let aborted: Result<(), ()> = host.transaction(|host, tx| {
            assert_eq!(old_slot.insert(host, tx, &resource, 1), 1);
            player
                .open_mod_menu(
                    host,
                    mod_api::ModMenuOpen {
                        menu: key,
                        title: mod_api::Text::literal("Replacement"),
                        extra: Vec::new(),
                        data: Vec::new(),
                        pos: None,
                        opening_hand: Some(Hand::Off),
                        replace: MenuReplace::InPlace,
                    },
                )
                .unwrap()
                .unwrap();
            let new = player.container_menu(host).unwrap();
            let new_slot = ItemPlace::MenuSlot { menu: new, slot: 1 };
            assert_eq!(new_slot.insert(host, tx, &resource, 4), 4);
            Err(())
        });
        assert_eq!(aborted, Err(()));
        let new = game.player_container_menu(player).unwrap();
        assert_eq!(old.slot(game, 1), None);
        assert_eq!(new.slot(game, 1), Some(None), "no leakage between windows");
        game.run_native_menu_events();
        let guest = &game.lan.as_ref().unwrap().hub.guests[0];
        assert_eq!(
            guest
                .inventory
                .iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>(),
            [ItemStack::new("minecraft:stick", 2)],
            "the old close returned only the committed value"
        );
    });
}

#[test]
#[ignore = "daily"]
fn the_part_closes_the_menu_when_the_opening_stack_leaves_its_hand() {
    with_owned_menu(false, |game, rx, key| {
        let (window, menu) = open(game, key);
        let change_hand = |game: &mut Game<'_>, change: fn(&mut ItemStack)| {
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                change(g.offhand.as_mut().unwrap());
            });
        };
        change_hand(game, |s| s.count = 3);
        click(game, window, 1);
        assert_eq!(
            menu.container_id(game),
            Some(window),
            "count does not affect the comparison"
        );

        // Same item, different components: the opening stack is no longer held.
        change_hand(game, |s| {
            s.data_mut().custom_name = Some(sunrise_core::nbt::Tag::String("Different".into()));
        });
        click(game, window, 1);
        assert_eq!(menu.container_id(game), None);
        assert!(sent(&rx).iter().any(|p| matches!(p,
            C::ContainerClose(p) if p.container == window
        )));

        let (window, menu) = open(game, key);
        game.lan
            .as_mut()
            .unwrap()
            .hub
            .guests
            .update(0, |g| g.offhand = None);
        click(game, window, 1);
        assert_eq!(menu.container_id(game), None);
        assert!(sent(&rx).iter().any(|p| matches!(p,
            C::ContainerClose(p) if p.container == window
        )));
    });
}
