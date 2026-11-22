//! `menu_slot_rules` through a real `MenuPart` on the read-only `Host`: what a click asks, in
//! what order, how often, and what a click does without a listener, with a failing one and with
//! one that answers out of range. The fixtures need prepared resources.
use crate::game::native_entity_refs::tests::with_game;
use crate::game::native_guest_turn_tests::with_guests;
use mod_api::{Hooks, MenuPart, MenuSlotRulesEvent, ModEntry, Mods, Registrar};
use std::cell::RefCell;
use sunrise_client::{
    items::ItemStack,
    menu::{Input, Kind, Menu},
};

thread_local! {
    /// Every question the part was asked: slot, item and count.
    static ASKED: RefCell<Vec<(usize, String, u32)>> = const { RefCell::new(Vec::new()) };
    /// What the part answers for `maximum`, and whether it panics on dirt.
    static ANSWER: RefCell<(Option<u32>, bool)> = const { RefCell::new((None, false)) };
    /// Whether the guest 12 resolved through the `Host`, at each question.
    static SAW_GUEST: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
}

struct Rules;
#[mod_api::sunrise_part]
impl MenuPart for Rules {
    fn menu_slot_rules(&self, host: &dyn mod_api::Host, event: &mut MenuSlotRulesEvent<'_>) {
        SAW_GUEST.with(|saw| saw.borrow_mut().push(host.player(12).is_some()));
        let item = mod_api::__private::item_key_id(event.stack.item())
            .unwrap()
            .to_owned();
        ASKED.with(|asked| {
            asked
                .borrow_mut()
                .push((event.slot, item.clone(), event.stack.count()))
        });
        let (maximum, panic) = ANSWER.with(|answer| *answer.borrow());
        if panic && item == "minecraft:dirt" {
            panic!("a listener failed");
        }
        if let Some(maximum) = maximum {
            event.max_stack = maximum;
        }
    }
}
struct Listener;
#[mod_api::__subscriptions]
impl Hooks for Listener {
    fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        r.menu_part("minecraft:generic_9x3", Rules);
    }
}
/// A mod with no part for any menu.
struct Quiet;
#[mod_api::__subscriptions]
impl Hooks for Quiet {}

fn reset(maximum: Option<u32>, panic: bool) {
    SAW_GUEST.with(|saw| saw.borrow_mut().clear());
    ASKED.with(|asked| asked.borrow_mut().clear());
    ANSWER.with(|answer| *answer.borrow_mut() = (maximum, panic));
}
fn asked() -> Vec<(usize, String, u32)> {
    ASKED.with(|asked| asked.borrow().clone())
}

/// A chest menu with its type registered, empty, the cursor holding `count` of `item`.
fn chest(carried: (&str, u32)) -> Menu {
    let mut menu = Menu::new(Kind::Chest(3));
    menu.set_registry_id(Some("minecraft:generic_9x3"));
    menu.carried = Some(ItemStack::new(carried.0, carried.1));
    menu
}
/// What a pickup click on slot 0 leaves: the slot and the cursor.
fn pickup(game: &crate::game::Game<'_>, menu: &mut Menu) -> (u32, u32) {
    menu.click(&game.menu_env(), 0, 0, Input::Pickup).unwrap();
    (
        menu.slot(0).map_or(0, |s| s.count),
        menu.carried.as_ref().map_or(0, |s| s.count),
    )
}

fn with_mods(mods: Vec<ModEntry>, test: impl FnOnce()) {
    let mods: Mods = mods.into();
    crate::game::api_values::install_parts(&mods).unwrap();
    mod_platform::testing::with_mods(mods, test);
}

struct StagedReader;
#[mod_api::sunrise_part]
impl MenuPart for StagedReader {
    fn menu_slot_rules(&self, _: &dyn mod_api::Host, event: &mut MenuSlotRulesEvent<'_>) {
        use mod_api::MenuContainer::{Main, Player};
        assert!(event.context.slot(Main, 2).is_none()); // unknown item
        assert!(event.context.slot(Main, usize::MAX).is_none());
        assert!(event.context.slot(Player, 0).is_none()); // occupied, but not lent
        let ingredient = event.context.slot(Main, 1).unwrap();
        assert_eq!(
            mod_api::__private::item_key_id(ingredient.item()),
            Some("minecraft:oak_planks")
        );
        event.max_stack = ingredient.count();
    }
}

struct StagedListener;
#[mod_api::__subscriptions]
impl Hooks for StagedListener {
    fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        r.menu_part("minecraft:generic_9x3", StagedReader);
    }
}

#[test]
#[ignore = "daily"]
fn staged_slot_rule_reads_hide_unrepresentable_and_player_stacks() {
    with_mods(
        vec![ModEntry::new("rules", Box::new(StagedListener))],
        || {
            with_game(|game| {
                let mut menu = chest(("minecraft:stone", 8));
                menu.load_slot(1, Some(ItemStack::new("minecraft:oak_planks", 3)));
                menu.load_slot(2, Some(ItemStack::new("test:unregistered", 1)));
                menu.set_player_item(0, Some(ItemStack::new("minecraft:stone", 1)));
                assert_eq!(pickup(game, &mut menu), (3, 5));
            });
        },
    );
}

#[test]
#[ignore = "daily"]
fn a_click_asks_the_part_for_each_slot_and_stack_it_consults_in_order() {
    with_mods(vec![ModEntry::new("rules", Box::new(Listener))], || {
        with_game(|game| {
            reset(None, false);
            let mut menu = chest(("minecraft:stone", 8));
            // Placement and capacity share one rule answer for the same slot and stack.
            assert_eq!(pickup(game, &mut menu), (8, 0));
            let stone = |_| (0, "minecraft:stone".to_owned(), 8);
            assert_eq!(asked(), [stone(())]);
            // Taking the stack back asks once: `Slot.allowModification` is `mayPickup` and
            // `mayPlace` of the slot's own stack, eight stone in slot 0.
            reset(None, false);
            menu.carried = None;
            assert_eq!(pickup(game, &mut menu), (0, 8));
            assert_eq!(asked(), [(0, "minecraft:stone".to_owned(), 8)]);
        });
    });
}

#[test]
#[ignore = "daily"]
fn the_answers_are_bounded_to_one_to_ninety_nine_and_the_items_limit() {
    with_mods(vec![ModEntry::new("rules", Box::new(Listener))], || {
        with_game(|game| {
            // A maximum of 0 holds one (the lower bound), 3 holds three, and a huge one holds what
            // the item stacks to: 64 stone, not 99 and not the answer.
            for (answer, held) in [(0, 1), (3, 3), (u32::MAX, 64)] {
                reset(Some(answer), false);
                let mut menu = chest(("minecraft:stone", 64));
                assert_eq!(pickup(game, &mut menu), (held, 64 - held), "{answer}");
            }
        });
    });
}

#[test]
#[ignore = "daily"]
fn a_failing_listener_leaves_the_slots_own_answers_and_the_next_click_works() {
    with_mods(vec![ModEntry::new("rules", Box::new(Listener))], || {
        with_game(|game| {
            reset(None, true);
            let mut menu = chest(("minecraft:dirt", 8));
            // The listener panics on dirt: the panic does not leave the click, and the slot's own
            // answers (it takes the stack) decide it.
            assert_eq!(pickup(game, &mut menu), (8, 0));
            assert!(
                asked()
                    .iter()
                    .all(|(slot, item, _)| (*slot, item.as_str()) == (0, "minecraft:dirt"))
            );
            // The next built-in click works.
            reset(None, true);
            let mut next = chest(("minecraft:stone", 2));
            assert_eq!(pickup(game, &mut next), (2, 0));
        });
    });
}

#[test]
#[ignore = "daily"]
fn without_a_part_nothing_is_asked_and_the_click_is_the_same() {
    // The same click with no mod installed, with a mod that has no part, and with a part on
    // another menu type: the final state is identical and the part is never asked.
    let run = |game: &crate::game::Game<'_>, registry: &str| {
        let mut menu = chest(("minecraft:stone", 8));
        menu.set_registry_id(Some(registry));
        pickup(game, &mut menu)
    };
    let baseline = RefCell::new(None);
    with_game(|game| {
        reset(None, false);
        *baseline.borrow_mut() = Some(run(game, "minecraft:generic_9x3"));
        assert_eq!(asked(), []);
    });
    with_mods(vec![ModEntry::new("quiet", Box::new(Quiet))], || {
        with_game(|game| {
            reset(None, false);
            assert_eq!(Some(run(game, "minecraft:generic_9x3")), *baseline.borrow());
            assert_eq!(asked(), []);
        });
    });
    with_mods(vec![ModEntry::new("rules", Box::new(Listener))], || {
        with_game(|game| {
            reset(None, false);
            // A menu type the part is not attached to: not asked, same answer.
            assert_eq!(Some(run(game, "minecraft:generic_9x6")), *baseline.borrow());
            assert_eq!(asked(), []);
        });
    });
}

#[test]
#[ignore = "daily"]
fn a_lan_hub_click_asks_the_same_questions_in_the_same_order() {
    use sunrise_client::lan::menu::Slots;
    use sunrise_protocol::packets::play::{ContainerClick, Serverbound};
    with_mods(vec![ModEntry::new("rules", Box::new(Listener))], || {
        with_guests(|game| {
            let pos = (1, 64, 1);
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            game.world.set(pos.0, pos.1, pos.2, stone).unwrap();
            game.world.block_entities.insert(
                pos,
                sunrise_client::containers::BlockEntity::Chest(vec![None; 27]),
            );
            let host = game.lan.as_mut().unwrap();
            host.hub.open_container(
                12,
                "minecraft:generic_9x3",
                sunrise_core::nbt::Tag::String("test".into()),
                pos,
                Slots::Block(pos),
                vec![None; 27],
                None,
            );
            let window = host.hub.guests[0].menu.as_ref().unwrap().window;
            host.hub.guests.update(0, |guest| {
                guest.carried = Some(ItemStack::new("minecraft:stone", 8))
            });
            reset(None, false);
            game.lan_guest_packet(
                12,
                Serverbound::ContainerClick(ContainerClick {
                    container: window,
                    state: 0,
                    slot: 0,
                    button: 0,
                    input: 0,
                    changed: Vec::new(),
                    carried: None,
                }),
            );
            let lan = asked();
            // The hub's click ran with the LAN host lifted out of the game, and is back. The
            // `Host` read meanwhile answered as if no LAN were open: the guest is not found.
            assert!(game.lan.is_some());
            assert_eq!(SAW_GUEST.with(|saw| saw.borrow().clone()), [false]);
            assert!(game.player(12).is_some());
            // The local click of the same state asks the same, in the same order.
            reset(None, false);
            let mut menu = chest(("minecraft:stone", 8));
            assert_eq!(pickup(game, &mut menu), (8, 0));
            assert_eq!(lan, asked());
            assert_eq!(lan.len(), 1);
            // A click made with the game whole (the local route) reaches the guest.
            assert_eq!(SAW_GUEST.with(|saw| saw.borrow().clone()), [true]);
            let placed = game.world.block_entities.get(&pos).unwrap().slots()[0]
                .as_ref()
                .map_or(0, |s| s.count);
            assert_eq!(placed, 8);
        });
    });
}

struct LiveSiblingPart;
#[mod_api::sunrise_part]
impl MenuPart for LiveSiblingPart {
    fn menu_slot_rules(&self, _: &dyn mod_api::Host, event: &mut MenuSlotRulesEvent<'_>) {
        if event.container == mod_api::MenuContainer::Main && event.slot == 1 {
            event.may_pickup = event.stack.count() == 3;
        }
        if event.container == mod_api::MenuContainer::Main && event.slot == 0 {
            assert_eq!(event.context.player, Some(12));
            assert_eq!(event.context.pos, Some([1, 64, 1]));
            if let Some(sibling) = event.context.slot(mod_api::MenuContainer::Main, 1) {
                event.max_stack = sibling.count();
                event.may_pickup = sibling.count() >= 4;
            } else {
                event.may_place = false;
                event.may_pickup = false;
            }
        }
    }
}

struct LiveSiblingListener;
#[mod_api::__subscriptions]
impl Hooks for LiveSiblingListener {
    fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        r.menu_part("minecraft:generic_9x1", LiveSiblingPart);
        let helmet = r.item_key("minecraft:iron_helmet");
        r.item_part(helmet, GuestFeetOnly);
    }
}

struct GuestFeetOnly;
#[mod_api::sunrise_part]
impl mod_api::ItemPart for GuestFeetOnly {
    fn can_equip(&self, host: &dyn mod_api::Host, event: &mut mod_api::CanEquipEvent<'_>) {
        assert_eq!(host.player(12).unwrap().entity(), event.entity.entity());
        event.can_equip = event.slot == "feet";
    }
}

/// An admitted block-backed menu, with registration and storage scoped to this game.
fn open_live_rules_fixture(
    game: &mut crate::game::Game<'_>,
    menu_type: &'static sunrise_client::containers::ModMenuType,
    items: Vec<Option<ItemStack>>,
) {
    use std::sync::Arc;
    use sunrise_client::{
        chunk::Chunk,
        containers::{BlockEntity, ModBlockEntity, ModBlockEntityFields, ModContainer},
        lan::menu::Slots,
    };
    let pos = (1, 64, 1);
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    let stone_state = game
        .pack
        .assets
        .registry
        .default_state("minecraft:stone")
        .unwrap();
    game.world.set(pos.0, pos.1, pos.2, stone_state).unwrap();
    let state = game.world.block(pos.0, pos.1, pos.2).0;
    game.world.block_entities.set_mod_containers(
        [(
            state,
            Arc::new(ModContainer {
                slots: menu_type.slots,
                ..Default::default()
            }),
        )]
        .into_iter()
        .collect(),
    );
    let entity = ModBlockEntity::new(
        "test:box".into(),
        ModBlockEntityFields {
            items,
            ..Default::default()
        },
    );
    game.world
        .block_entities
        .insert(pos, BlockEntity::Mod(Box::new(entity)));

    let host = game.lan.as_mut().unwrap();
    host.hub.open_container(
        12,
        menu_type.id,
        sunrise_core::nbt::Tag::String("box".into()),
        pos,
        Slots::Block(pos),
        vec![None; menu_type.slots],
        None,
    );
    host.hub
        .update_guest(12, |guest| {
            guest.menu.as_mut().unwrap().mod_type = Some(menu_type);
        })
        .unwrap();
}

#[test]
#[ignore = "daily"]
fn live_slot_queries_read_live_siblings_limits_acting_armor_and_stale_handles() {
    use crate::game::api_values::TryIntoApi;
    use sunrise_client::{
        command::args::GameMode,
        containers::{BlockEntity, ModLinks, ModMenuType},
        lan::menu::Slots,
    };

    let menu_type: &'static ModMenuType = Box::leak(Box::new(ModMenuType {
        id: "minecraft:generic_9x1",
        owner: "test",
        slots: 9,
        player_inventory: true,
        data_slots: 0,
        owned_slots: None,
        results: Vec::new(),
        inputs: Vec::new(),
        crafting: None,
        links: Some(ModLinks { from: 1, count: 2 }),
    }));

    with_mods(
        vec![ModEntry::new("live_sibling", Box::new(LiveSiblingListener))],
        || {
            with_guests(|game| {
                let pos = (1, 64, 1);
                let mut items = vec![None; 9];
                items[0] = Some(ItemStack::new("minecraft:stone", 2));
                items[1] = Some(ItemStack::new("minecraft:oak_planks", 55)); // raw, not the linked stack
                items[2] = Some(ItemStack::new("minecraft:stone", 7)); // unbound, not a click target
                open_live_rules_fixture(game, menu_type, items);
                let linked = (1, 64, 2);
                game.world.block_entities.insert(
                    linked,
                    BlockEntity::Chest(vec![Some(ItemStack::new("minecraft:oak_planks", 3))]),
                );
                game.lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .update_guest(12, |guest| {
                        guest.menu.as_mut().unwrap().links.push(linked);
                    })
                    .unwrap();

                let player = game.player(12).unwrap();
                let menu = game.player_container_menu(player).unwrap();
                let stone_api = ItemStack::new("minecraft:stone", 1).try_into_api().unwrap();

                // Block-backed mod menu queries take read-only `&dyn Host` and observe live
                // sibling slots without reopening.
                let read: &dyn mod_api::Host = game;
                assert_eq!(menu.slot_holds(read, 0, &stone_api), Some(3));
                assert_eq!(menu.slot_gives(read, 0), Some(false));
                assert_eq!(menu.slot_gives(read, 1), Some(true));

                assert_eq!(menu.slot_holds(read, 2, &stone_api), Some(0));
                assert_eq!(menu.slot_gives(read, 2), Some(false));
                assert!(matches!(
                    menu.slot_backing(read, 1),
                    Some(mod_api::MenuSlotBacking::BlockEntity {
                        pos: [1, 64, 2],
                        slot: 0,
                        ..
                    })
                ));
                assert_eq!(menu.slot_backing(read, 2), None);

                // Direct `ItemPlace::MenuSlot` writes reach the bound linked container and
                // refuse unbound linked indices without touching the opener's shadow slots.
                {
                    use mod_api::{HostTransaction, ItemPlace, ItemResource, ServerHost};
                    let planks = ItemResource::of(
                        &ItemStack::new("minecraft:oak_planks", 1)
                            .try_into_api()
                            .unwrap(),
                    );
                    let stone_res = ItemResource::of(&stone_api);
                    let bound = ItemPlace::MenuSlot { menu, slot: 1 };
                    let unbound = ItemPlace::MenuSlot { menu, slot: 2 };
                    let host: &mut dyn ServerHost = game;
                    let moved = host
                        .transaction(|host, tx| {
                            Ok::<_, ()>((
                                bound.extract(host, tx, &planks, 1),
                                bound.insert(host, tx, &planks, 2),
                                unbound.extract(host, tx, &stone_res, 1),
                                unbound.insert(host, tx, &stone_res, 1),
                            ))
                        })
                        .unwrap();
                    assert_eq!(moved, (1, 2, 0, 0));
                }
                assert_eq!(
                    game.world.block_entities.get(&linked).unwrap().slots()[0]
                        .as_ref()
                        .map(|s| s.count),
                    Some(4)
                );
                assert_eq!(
                    game.world.block_entities.get(&pos).unwrap().slots()[1]
                        .as_ref()
                        .map(|s| s.count),
                    Some(55)
                );
                assert_eq!(
                    game.world.block_entities.get(&pos).unwrap().slots()[2]
                        .as_ref()
                        .map(|s| s.count),
                    Some(7)
                );

                game.world
                    .block_entities
                    .get_mut(&linked)
                    .unwrap()
                    .slots_mut()[0] = Some(ItemStack::new("minecraft:oak_planks", 5));
                assert_eq!(menu.slot_holds(game, 0, &stone_api), Some(5));
                assert_eq!(menu.slot_gives(game, 0), Some(true));

                // Block container_stack_limit caps container slots without capping player slots.
                game.world.set_container_stack_limit(pos, 2);
                assert_eq!(menu.slot_holds(game, 0, &stone_api), Some(2));
                assert_eq!(menu.slot_holds(game, 9, &stone_api), Some(64));

                // Double chest applies each half's container_stack_limit independently,
                // and replacing the menu invalidates the previous handle.
                let left = (2, 64, 1);
                let right = (3, 64, 1);
                game.world
                    .block_entities
                    .insert(left, BlockEntity::Chest(vec![None; 27]));
                game.world
                    .block_entities
                    .insert(right, BlockEntity::Chest(vec![None; 27]));
                game.world.set_container_stack_limit(left, 5);
                game.world.set_container_stack_limit(right, 11);
                game.lan.as_mut().unwrap().hub.open_container(
                    12,
                    "minecraft:generic_9x6",
                    sunrise_core::nbt::Tag::String("double".into()),
                    left,
                    Slots::Double(left, right),
                    vec![None; 54],
                    None,
                );
                assert_eq!(menu.slot_holds(game, 0, &stone_api), None);
                assert_eq!(menu.slot_gives(game, 0), None);

                let double_menu = game.player_container_menu(player).unwrap();
                assert_eq!(double_menu.slot_holds(game, 0, &stone_api), Some(5));
                assert_eq!(double_menu.slot_holds(game, 27, &stone_api), Some(11));
                assert_eq!(double_menu.slot_holds(game, 54, &stone_api), Some(64));

                game.lan.as_mut().unwrap().hub.close_container(12);
                assert_eq!(double_menu.slot_holds(game, 0, &stone_api), None);
                assert_eq!(double_menu.slot_gives(game, 0), None);

                // Kind::Inventory observes acting-turn bound armor and creative mode.
                let inv = game.player_container_menu(player).unwrap();
                let mut bound_helmet = ItemStack::new("minecraft:diamond_helmet", 1);
                bound_helmet.set_crafting_enchantments(vec![("minecraft:binding_curse".into(), 1)]);
                game.as_guest(12, |game| {
                    let helmet = ItemStack::new("minecraft:iron_helmet", 1)
                        .try_into_api()
                        .unwrap();
                    assert_eq!(inv.slot_holds(game, 5, &helmet), Some(0));
                    assert_eq!(inv.slot_holds(game, 8, &helmet), Some(1));
                    game.armor[0] = Some(bound_helmet);
                    assert_eq!(inv.slot_gives(game, 5), Some(false));
                    assert_eq!(inv.slot_gives(game, 6), Some(true));
                    game.set_mode(GameMode::Creative);
                    assert_eq!(inv.slot_gives(game, 5), Some(true));
                    game.set_mode(GameMode::Survival);
                })
                .unwrap();
                assert_eq!(inv.slot_gives(game, 5), Some(false));
            });
        },
    );
}

struct LiveReusePart;
#[mod_api::sunrise_part]
impl MenuPart for LiveReusePart {
    fn menu_slot_rules(&self, _: &dyn mod_api::Host, event: &mut MenuSlotRulesEvent<'_>) {
        let item = mod_api::__private::item_key_id(event.stack.item())
            .unwrap()
            .to_owned();
        ASKED.with(|asked| {
            asked
                .borrow_mut()
                .push((event.slot, item, event.stack.count()))
        });
        if event.container == mod_api::MenuContainer::Main && event.slot == 0 {
            event.may_pickup = event
                .context
                .slot(mod_api::MenuContainer::Main, 1)
                .is_some_and(|s| s.count() >= 4);
        }
    }
}

struct LiveReuseListener;
#[mod_api::__subscriptions]
impl Hooks for LiveReuseListener {
    fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        r.menu_part("minecraft:generic_9x1", LiveReusePart);
    }
}

#[test]
#[ignore = "daily"]
fn mod_menu_slot_gives_reuses_recorded_answer_in_click_asked() {
    use sunrise_client::containers::ModMenuType;

    let menu_type: &'static ModMenuType = Box::leak(Box::new(ModMenuType {
        id: "minecraft:generic_9x1",
        owner: "test",
        slots: 9,
        player_inventory: true,
        data_slots: 0,
        owned_slots: None,
        results: Vec::new(),
        inputs: Vec::new(),
        crafting: None,
        links: None,
    }));

    with_mods(
        vec![ModEntry::new("live_reuse", Box::new(LiveReuseListener))],
        || {
            with_guests(|game| {
                let pos = [1, 64, 1];
                let stone = ItemStack::new("minecraft:stone", 8);

                // Refused pickup: mod_menu_slot_gives asks once, click_asked reuses it.
                reset(None, false);
                let mut items = vec![None; 9];
                items[0] = Some(stone.clone());
                items[1] = Some(ItemStack::new("minecraft:oak_planks", 2));
                open_live_rules_fixture(game, menu_type, items.clone());
                let ask_pickup = |game: &crate::game::Game<'_>| {
                    let guest = game.lan.as_ref().unwrap().hub.guest(12).unwrap();
                    let backing = game.menu_slot_backing_for(guest, 0).unwrap();
                    game.with_live_slot_rules(guest, backing, 0, |rules, env, held| {
                        rules.ask_pickup(env, 0, held)
                    })
                    .unwrap()
                };
                let (gives, asked_refused) = ask_pickup(game);
                assert!(!gives);
                assert!(!asked_refused.is_empty());
                assert_eq!(asked(), [(0, "minecraft:stone".to_owned(), 8)]);

                let env = sunrise_client::menu::Env {
                    player: Some(12),
                    ..game.menu_env()
                };
                let mut menu = Menu::new(Kind::Mod(menu_type));
                menu.set_registry_id(Some(menu_type.id));
                menu.set_block_pos(Some(pos));
                menu.load_container(&env, &items);
                menu.click_asked(&env, 0, 0, Input::QuickMove, asked_refused)
                    .unwrap();
                assert_eq!(asked(), [(0, "minecraft:stone".to_owned(), 8)]);
                assert_eq!(menu.slot(0), Some(&stone));

                // Allowed pickup: mod_menu_slot_gives asks once, click_asked reuses it
                // without invoking menu_slot_rules a second time.
                reset(None, false);
                items[1] = Some(ItemStack::new("minecraft:oak_planks", 5));
                game.world
                    .block_entities
                    .get_mut(&(1, 64, 1))
                    .unwrap()
                    .slots_mut()[1] = items[1].clone();
                let (gives, asked_allowed) = ask_pickup(game);
                let env = sunrise_client::menu::Env {
                    player: Some(12),
                    ..game.menu_env()
                };
                assert!(gives);
                assert!(!asked_allowed.is_empty());
                assert_eq!(asked(), [(0, "minecraft:stone".to_owned(), 8)]);

                let mut menu = Menu::new(Kind::Mod(menu_type));
                menu.set_registry_id(Some(menu_type.id));
                menu.set_block_pos(Some(pos));
                menu.load_container(&env, &items);
                menu.click_asked(&env, 0, 0, Input::Pickup, asked_allowed)
                    .unwrap();
                assert_eq!(asked(), [(0, "minecraft:stone".to_owned(), 8)]);
                assert_eq!(menu.slot(0), None);
                assert_eq!(menu.carried.as_ref(), Some(&stone));

                // Changing the read sibling slot invalidates the recorded answer and re-asks.
                reset(None, false);
                let (gives, asked_stale) = ask_pickup(game);
                assert!(gives);
                assert_eq!(asked().len(), 1);

                let mut changed = items.clone();
                changed[1] = Some(ItemStack::new("minecraft:oak_planks", 1));
                let mut menu = Menu::new(Kind::Mod(menu_type));
                menu.set_registry_id(Some(menu_type.id));
                menu.set_block_pos(Some(pos));
                menu.load_container(&env, &changed);
                menu.click_asked(&env, 0, 0, Input::QuickMove, asked_stale)
                    .unwrap();
                assert_eq!(asked().len(), 2);
                assert_eq!(menu.slot(0), Some(&stone));
            });
        },
    );
}

#[test]
#[ignore = "daily"]
fn cached_machine_menu_unhides_linked_slot_when_storage_binds() {
    use sunrise_client::{
        containers::{BlockEntity, ModLinks, ModMenuType},
        menu::Asked,
    };
    use sunrise_protocol::packets::play::ContainerClick;

    let menu_type: &'static ModMenuType = Box::leak(Box::new(ModMenuType {
        id: "minecraft:generic_9x1",
        owner: "test",
        slots: 9,
        player_inventory: true,
        data_slots: 0,
        owned_slots: None,
        results: Vec::new(),
        inputs: Vec::new(),
        crafting: None,
        links: Some(ModLinks { from: 1, count: 2 }),
    }));

    with_guests(|game| {
        open_live_rules_fixture(game, menu_type, vec![None; 9]);
        let linked_a = (1, 64, 2);
        let linked_b = (1, 64, 3);
        game.world.block_entities.insert(
            linked_a,
            BlockEntity::Chest(vec![Some(ItemStack::new("minecraft:oak_planks", 3))]),
        );
        game.lan
            .as_mut()
            .unwrap()
            .hub
            .update_guest(12, |guest| {
                guest.menu.as_mut().unwrap().links.push(linked_a);
            })
            .unwrap();
        let window = game
            .lan
            .as_ref()
            .unwrap()
            .hub
            .guest(12)
            .unwrap()
            .menu
            .as_ref()
            .unwrap()
            .window;
        let click_slot = |slot: i16| ContainerClick {
            container: window,
            state: 0,
            slot,
            button: 0,
            input: 0,
            changed: Vec::new(),
            carried: None,
        };

        // First click caches the machine `Menu` while slot 2 is unbound (hidden).
        assert_eq!(
            game.lan_machine_click_asked(12, &click_slot(2), Asked::default()),
            Some(true)
        );
        assert!(
            game.lan
                .as_ref()
                .unwrap()
                .hub
                .guest(12)
                .unwrap()
                .carried
                .is_none()
        );

        // Bind a second container to slot 2 in the same open window without reopening.
        game.world.block_entities.insert(
            linked_b,
            BlockEntity::Chest(vec![Some(ItemStack::new("minecraft:dirt", 4))]),
        );
        game.lan
            .as_mut()
            .unwrap()
            .hub
            .update_guest(12, |guest| {
                guest.menu.as_mut().unwrap().links.push(linked_b);
            })
            .unwrap();

        assert_eq!(
            game.lan_machine_click_asked(12, &click_slot(2), Asked::default()),
            Some(true)
        );
        let carried = game
            .lan
            .as_ref()
            .unwrap()
            .hub
            .guest(12)
            .unwrap()
            .carried
            .as_ref()
            .expect("slot 2 should unhide on the cached machine menu");
        assert_eq!(carried.item.as_ref(), "minecraft:dirt");
        assert_eq!(carried.count, 4);
        assert!(game.world.block_entities.get(&linked_b).unwrap().slots()[0].is_none());
    });
}

struct QuickMoveRoutedPart;
#[mod_api::sunrise_part]
impl mod_api::BlockPart for QuickMoveRoutedPart {}
#[mod_api::sunrise_part]
impl mod_api::BlockEntityPart for QuickMoveRoutedPart {
    fn menu_quick_move(
        &self,
        _: &mut dyn mod_api::ServerHost,
        event: &mut mod_api::MenuQuickMoveEvent<'_>,
    ) -> mod_api::Flow {
        event.entity.items.resize(9, None);
        if event.slot == 1
            && let Some(stack) = event.stack.as_mut()
            && stack.count() >= 2
        {
            let mut moved = stack.clone();
            moved.set_count(2).unwrap();
            event.entity.items[0] = Some(moved);
            stack.set_count(stack.count() - 2).unwrap();
            return mod_api::Flow::Cancel;
        }
        if event.slot == 7 {
            if event.entity.items[8].is_none() {
                event.entity.items[8] = event.stack.take();
            }
            return mod_api::Flow::Cancel;
        }
        mod_api::Flow::Continue
    }
}

struct QuickMoveRoutedListener;
#[mod_api::__subscriptions]
impl Hooks for QuickMoveRoutedListener {
    fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        let kind = r.block_entity_type("box").blocks(&[]).build();
        r.block_entity_part(&kind, QuickMoveRoutedPart).unwrap();
    }
}

#[test]
#[ignore = "daily"]
fn quick_move_routes_bound_linked_and_craft_result_slots() {
    use sunrise_client::containers::{BlockEntity, ModCrafting, ModLinks, ModMenuType};
    use sunrise_protocol::packets::play::ContainerClick;

    let menu_type: &'static ModMenuType = Box::leak(Box::new(ModMenuType {
        id: "minecraft:generic_9x1",
        owner: "test",
        slots: 9,
        player_inventory: true,
        data_slots: 0,
        owned_slots: None,
        results: Vec::new(),
        inputs: Vec::new(),
        crafting: Some(ModCrafting {
            grid: 3,
            size: 2,
            result: 7,
        }),
        links: Some(ModLinks { from: 1, count: 2 }),
    }));

    with_mods(
        vec![ModEntry::new("test", Box::new(QuickMoveRoutedListener))],
        || {
            with_guests(|game| {
                let pos = (1, 64, 1);
                let linked = (1, 64, 2);
                let mut items = vec![None; 9];
                items[1] = Some(ItemStack::new("minecraft:oak_planks", 55));
                items[2] = Some(ItemStack::new("minecraft:stone", 7));
                items[3] = Some(ItemStack::new("minecraft:oak_log", 2));
                open_live_rules_fixture(game, menu_type, items);
                game.world.block_entities.insert(
                    linked,
                    BlockEntity::Chest(vec![Some(ItemStack::new("minecraft:dirt", 6))]),
                );
                game.lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .update_guest(12, |guest| {
                        guest.menu.as_mut().unwrap().links.push(linked);
                    })
                    .unwrap();
                game.lan_mod_crafting_tick();

                let window = game
                    .lan
                    .as_ref()
                    .unwrap()
                    .hub
                    .guest(12)
                    .unwrap()
                    .menu
                    .as_ref()
                    .unwrap()
                    .window;
                let shift_click = |slot: i16| ContainerClick {
                    container: window,
                    state: 0,
                    slot,
                    button: 0,
                    input: 1,
                    changed: Vec::new(),
                    carried: None,
                };

                // Bound linked slot: reads and settles to the linked chest, never opener slot 1.
                assert_eq!(game.lan_mod_quick_move(12, &shift_click(1)), Some(true));
                assert_eq!(
                    game.world.block_entities.get(&pos).unwrap().slots()[0]
                        .as_ref()
                        .map(|s| (s.item.as_ref(), s.count)),
                    Some(("minecraft:dirt", 2))
                );
                assert_eq!(
                    game.world.block_entities.get(&pos).unwrap().slots()[1]
                        .as_ref()
                        .map(|s| s.count),
                    Some(55)
                );
                assert_eq!(
                    game.world.block_entities.get(&linked).unwrap().slots()[0]
                        .as_ref()
                        .map(|s| (s.item.as_ref(), s.count)),
                    Some(("minecraft:dirt", 4))
                );

                // Unbound linked slot does not fire `menu_quick_move` or touch opener slot 2.
                assert_eq!(game.lan_mod_quick_move(12, &shift_click(2)), None);
                assert_eq!(
                    game.world.block_entities.get(&pos).unwrap().slots()[2]
                        .as_ref()
                        .map(|s| s.count),
                    Some(7)
                );

                // Craft result slot: taking consumes one craft from the grid without writing
                // to `entity.items[7]`, and settles the next result for the remaining log.
                assert_eq!(game.lan_mod_quick_move(12, &shift_click(7)), Some(true));
                assert_eq!(
                    game.world.block_entities.get(&pos).unwrap().slots()[8]
                        .as_ref()
                        .map(|s| (s.item.as_ref(), s.count)),
                    Some(("minecraft:oak_planks", 4))
                );
                assert!(game.world.block_entities.get(&pos).unwrap().slots()[7].is_none());
                assert_eq!(
                    game.world.block_entities.get(&pos).unwrap().slots()[3]
                        .as_ref()
                        .map(|s| s.count),
                    Some(1)
                );
                let player = game.player(12).unwrap();
                let menu = game.player_container_menu(player).unwrap();
                let next_result = menu.slot(game, 7).unwrap().unwrap();
                assert_eq!(
                    mod_api::__private::item_key_id(next_result.item()),
                    Some("minecraft:oak_planks")
                );

                // Handled without taking leaves the grid and `entity.items[7]` untouched.
                assert_eq!(game.lan_mod_quick_move(12, &shift_click(7)), Some(true));
                assert_eq!(
                    game.world.block_entities.get(&pos).unwrap().slots()[3]
                        .as_ref()
                        .map(|s| s.count),
                    Some(1)
                );
                assert!(game.world.block_entities.get(&pos).unwrap().slots()[7].is_none());
            });
        },
    );
}
