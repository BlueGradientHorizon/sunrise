use super::tests::{MOD_MENU, mod_menu_guest, sent};
use crate::game::{Game, api_values, native_entity_refs::tests::with_game};
use mod_api::{
    Hand, Hooks, HostTransaction, ItemPlace, ItemResource, MenuClickedEvent, MenuClosedEvent,
    MenuKey, MenuPart, MenuRef, MenuReplace, MenuSlotBackRefusal, MenuSlotBacking, MenuSlotPlace,
    MenuStillValidEvent, ModEntry, Mods, Registrar, ServerHost,
};
use std::sync::mpsc;
use sunrise_client::{items::ItemStack, lan::Out};
use sunrise_protocol::packets::play::{
    Clientbound as C, ContainerClick, ContainerClose, Serverbound as S,
};

struct Part;
#[mod_api::sunrise_part]
impl MenuPart for Part {
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

    fn menu_clicked(&self, host: &mut dyn ServerHost, event: &mut MenuClickedEvent) {
        if event.slot != 0 {
            return;
        }
        let menu = host.menu(event.uuid, event.window).unwrap();
        let cursor = menu.carried(host).flatten().unwrap();
        host.transaction(|host, tx| {
            ItemPlace::MenuSlot { menu, slot: 0 }.insert(host, tx, &ItemResource::of(&cursor), 1);
            Ok::<_, ()>(())
        })
        .unwrap();
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
                key,
                mod_api::Text::literal("Owned slots"),
                Vec::new(),
                Vec::new(),
                None,
                Some(Hand::Off),
                MenuReplace::CloseFirst,
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
    });
}

#[test]
#[ignore = "daily"]
fn a_click_copies_into_an_owned_slot_and_syncs_without_consuming_the_cursor() {
    with_owned_menu(false, |game, rx, key| {
        let (window, menu) = open(game, key);
        game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
            g.carried = Some(ItemStack::new("minecraft:stick", 7));
        });
        click(game, window, 0);
        let stack = menu.slot(game, 0).flatten().unwrap();
        let cursor = menu.carried(game).flatten().unwrap();
        assert_eq!(cursor.item(), stack.item());
        assert_eq!(stack.count(), 1);
        assert_eq!(cursor.count(), 7);
        sent(&rx);
        game.lan.as_mut().unwrap().hub.sync_containers(&game.world);
        assert!(sent(&rx).iter().any(|p| matches!(p,
            C::ContainerSetSlot(slot) if slot.container == window && slot.slot == 0 && slot.item.is_some()
        )));
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
    });
}
