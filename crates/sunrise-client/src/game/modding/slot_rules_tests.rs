//! `menu_slot_rules` through a real `MenuPart` on the read-only `Host`: what a click asks, in
//! what order, how often, and what a click does without a listener, with a failing one and with
//! one that answers out of range. The fixtures need prepared resources.
use crate::game::native_entity_refs::tests::with_game;
use crate::game::native_guest_turn_tests::with_guests;
use mod_api::{Hooks, MenuPart, MenuSlotRulesEvent, ModEntry, Mods, Registrar};
use std::cell::RefCell;
use sunrise_client::{
    items::ItemStack,
    lan::menu::Slots,
    menu::{Input, Kind, Menu},
};
use sunrise_protocol::packets::play::{ContainerClick, Serverbound};

const CHEST_POS: (i32, i32, i32) = (1, 64, 1);

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
        let sees_guest = host.player(12).is_some();
        SAW_GUEST.with(|saw| saw.borrow_mut().push(sees_guest));
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
            event.may_place = false;
            event.max_stack = 1;
            panic!("a listener failed");
        }
        if let Some(maximum) = maximum.or_else(|| sees_guest.then_some(3)) {
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

fn with_hub_chest(test: impl FnOnce(&mut crate::game::Game<'_>, mod_api::MenuRef, i32)) {
    with_guests(|game| {
        game.reset_entity_residency();
        let (x, y, z) = CHEST_POS;
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
        game.world.set(x, y, z, stone).unwrap();
        game.world.block_entities.insert(
            CHEST_POS,
            sunrise_client::containers::BlockEntity::Chest(vec![None; 27]),
        );
        let host = game.lan.as_mut().unwrap();
        host.hub.open_container(
            12,
            "minecraft:generic_9x3",
            sunrise_core::nbt::Tag::String("test".into()),
            CHEST_POS,
            Slots::Block(CHEST_POS),
            vec![None; 27],
            None,
        );
        let window = host.hub.guests[0].menu.as_ref().unwrap().window;
        let menu = game
            .player_container_menu(game.player(12).unwrap())
            .unwrap();
        test(game, menu, window);
    });
}

fn hub_pickup(game: &mut crate::game::Game<'_>, window: i32, item: &str) {
    game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
        guest.carried = Some(ItemStack::new(item, 8));
    });
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
fn a_panicking_menu_part_loses_only_its_answer_and_is_disabled() {
    struct Capacity;
    #[mod_api::sunrise_part]
    impl MenuPart for Capacity {
        fn menu_slot_rules(&self, _: &dyn mod_api::Host, event: &mut MenuSlotRulesEvent<'_>) {
            event.max_stack = event.max_stack.min(3);
        }
    }
    thread_local! {
        static STACKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    struct Healthy;
    #[mod_api::__subscriptions]
    impl Hooks for Healthy {
        fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
            r.menu_part("minecraft:generic_9x3", Capacity);
        }
        fn item_stacked_on_other(
            &self,
            _: mod_api::HostSide<'_>,
            _: &mut mod_api::ItemStackedOnOtherEvent<'_>,
        ) -> mod_api::Flow {
            STACKED.set(true);
            mod_api::Flow::Continue
        }
    }
    with_mods(
        vec![
            ModEntry::new("rules", Box::new(Listener)),
            ModEntry::new("healthy", Box::new(Healthy)),
        ],
        || {
            with_hub_chest(|game, menu, window| {
                reset(None, true);
                for attempt in 0..2 {
                    // The rule panic occurs while planning the journaled pickup. It does not
                    // invalidate that separate action, and the healthy Part still caps the slot.
                    hub_pickup(game, window, "minecraft:dirt");
                    assert!(STACKED.replace(false));
                    assert_eq!(
                        menu.with_view(game, |view| {
                            (
                                view.slot(0).flatten().map(|stack| stack.count()),
                                view.carried().flatten().map(|stack| stack.count()),
                            )
                        }),
                        Some((Some(3), Some(5)))
                    );
                    let reports = mod_api::__private::take_hook_panics();
                    if attempt == 0 {
                        assert_eq!(asked(), [(0, "minecraft:dirt".to_owned(), 8)]);
                        assert_eq!(
                            reports,
                            [mod_api::__private::HookPanicReport {
                                mod_id: "rules",
                                hook: "menu_slot_rules",
                            }]
                        );
                    } else {
                        assert!(asked().is_empty());
                        assert!(reports.is_empty());
                    }
                    game.world
                        .block_entities
                        .get_mut(&CHEST_POS)
                        .unwrap()
                        .slots_mut()[0] = None;
                    reset(None, true);
                }
            });
        },
    );
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
    with_mods(vec![ModEntry::new("rules", Box::new(Listener))], || {
        with_hub_chest(|game, menu, window| {
            let offered = crate::game::api_values::TryIntoApi::try_into_api(&ItemStack::new(
                "minecraft:stone",
                8,
            ))
            .unwrap();
            reset(None, false);
            assert_eq!(
                menu.with_view(game, |view| view.slot_holds(0, &offered)),
                Some(Some(3))
            );
            assert_eq!(SAW_GUEST.with(|saw| saw.borrow().clone()), [true]);
            reset(None, false);
            hub_pickup(game, window, "minecraft:stone");
            let lan = asked();
            // Planning the Hub click sees the same live guest as the menu view.
            assert_eq!(SAW_GUEST.with(|saw| saw.borrow().clone()), [true]);
            assert!(game.player(12).is_some());
            // The local click of the same state asks the same, in the same order.
            reset(None, false);
            let mut menu = chest(("minecraft:stone", 8));
            assert_eq!(pickup(game, &mut menu), (3, 5));
            assert_eq!(lan, asked());
            assert_eq!(lan.len(), 1);
            // A click made with the game whole (the local route) reaches the guest.
            assert_eq!(SAW_GUEST.with(|saw| saw.borrow().clone()), [true]);
            let placed = game.world.block_entities.get(&CHEST_POS).unwrap().slots()[0]
                .as_ref()
                .map_or(0, |s| s.count);
            assert_eq!(placed, 3);
        });
    });
}
