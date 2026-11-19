//! `menu_slot_take` through a real `BlockEntityPart` on the block a mod menu type is open on, as
//! a LAN guest's clicks reach the server: what each take hands over, once per take; what is not a
//! take is silent; and a failing listener undoes its writes with the take. The fixtures need
//! prepared resources.
use crate::game::native_guest_turn_tests::with_guests;
use mod_api::{
    BlockEntityPart, BlockPart, Hooks, HostTransaction, ItemPlace, ItemResource, MenuPart,
    MenuSlotRulesEvent, MenuSlotTakeEvent, ModEntry, Mods, Registrar,
};
use std::{cell::RefCell, sync::Arc};
use sunrise_client::{
    chunk::Chunk,
    containers::{
        BlockEntity, ModBlockEntity, ModBlockEntityFields, ModContainer, ModCrafting, ModMenuType,
    },
    items::ItemStack,
    lan::menu::{ModCraft, Slots},
    menu::{Input, Kind, Menu},
};
use sunrise_protocol::packets::play::{ContainerClick, Serverbound};

/// The menu type the pot opens: a prepared registry slot (`minecraft:generic_9x3`) for a
/// synthetic mod menu type, so the guest's click names it.
const MENU: &str = "minecraft:generic_9x3";
const POS: (i32, i32, i32) = (1, 64, 1);
/// The pot's slot whose pickup rule refuses.
const KEPT: usize = 1;
/// The pot's result slot: it refuses placement.
const RESULT: usize = 2;
/// Slots of the pot's container: its hotbar 0 is menu slot `SLOTS + 27`.
const SLOTS: usize = 9;

/// A take as heard: player, menu, slot, what was taken and what was left (item and count).
type Heard = (u128, String, usize, (String, u32), Option<(String, u32)>);

thread_local! {
    static HEARD: RefCell<Vec<Heard>> = const { RefCell::new(Vec::new()) };
    /// Whether the listener puts one of the taken stack in the player's hotbar slot 8 and then
    /// fails.
    static FAIL: RefCell<bool> = const { RefCell::new(false) };
}

fn stack(stack: &mod_api::ItemStack) -> (String, u32) {
    let item = mod_api::__private::item_key_id(stack.item()).unwrap();
    (item.to_owned(), stack.count())
}

struct Pot;
#[mod_api::sunrise_part]
impl BlockPart for Pot {}
#[mod_api::sunrise_part]
impl BlockEntityPart for Pot {
    fn menu_slot_take(
        &self,
        host: &mut dyn mod_api::ServerHost,
        event: &mut MenuSlotTakeEvent<'_>,
    ) {
        let heard = (
            event.player,
            event.menu.to_owned(),
            event.slot,
            stack(event.taken),
            event.left.map(stack),
        );
        HEARD.with(|h| h.borrow_mut().push(heard));
        // A write through the host: one of the taken stack into the hotbar's last slot.
        let place = ItemPlace::PlayerSlot {
            player: event.player,
            slot: 8,
        };
        let one = ItemResource::of(event.taken);
        let put = host.transaction(|host, tx| Ok::<_, ()>(place.insert(host, tx, &one, 1)));
        assert_eq!(put, Ok(1));
        if FAIL.with(|fail| *fail.borrow()) {
            panic!("a broken listener");
        }
    }
}
/// The pot's pickup rule: slot `KEPT` gives nothing up.
struct Rules;
#[mod_api::sunrise_part]
impl MenuPart for Rules {
    fn menu_slot_rules(&self, _: &dyn mod_api::Host, event: &mut MenuSlotRulesEvent<'_>) {
        event.may_pickup &= event.slot != KEPT;
    }
}
struct TakeMod;
#[mod_api::__subscriptions]
impl Hooks for TakeMod {
    fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
        r.menu_part(MENU, Rules);
        let kind = r.block_entity_type("pot").blocks(&[]).build();
        r.block_entity_part(&kind, Pot).unwrap();
    }
}

fn pot_type(slots: usize, crafting: Option<ModCrafting>) -> &'static ModMenuType {
    Box::leak(Box::new(ModMenuType {
        id: MENU,
        slots,
        data_slots: 0,
        results: vec![RESULT],
        crafting,
        links: None,
    }))
}
fn item(name: &str, count: u32) -> Option<ItemStack> {
    Some(ItemStack::new(name, count))
}
fn stone(count: u32) -> Option<ItemStack> {
    item("minecraft:stone", count)
}
fn heard() -> Vec<Heard> {
    HEARD.with(|h| std::mem::take(&mut *h.borrow_mut()))
}
/// A take of `taken` stone from `slot`, `left` stone staying (0 for none), heard from guest 12.
fn took(slot: usize, taken: u32, left: u32) -> Heard {
    let stone = |count| ("minecraft:stone".to_owned(), count);
    let left = (left > 0).then(|| stone(left));
    (12, MENU.to_owned(), slot, stone(taken), left)
}

/// Guest 12 with the pot of menu type `menu_type` at `POS` open.
fn with_pot(menu_type: &'static ModMenuType, test: impl FnOnce(&mut crate::game::Game<'_>)) {
    let mods: Mods = vec![ModEntry::new("take", Box::new(TakeMod))].into();
    crate::game::api_values::install_parts(&mods).unwrap();
    crate::entry::install_client_hooks();
    mod_platform::testing::with_mods(mods, || {
        with_guests(|game| {
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            game.world.set(POS.0, POS.1, POS.2, stone).unwrap();
            let slots = menu_type.slots;
            let container = Arc::new(ModContainer {
                slots,
                ..Default::default()
            });
            let state = game.world.block(POS.0, POS.1, POS.2).0;
            let containers = [(state, container)].into_iter().collect();
            game.world.block_entities.set_mod_containers(containers);
            let fields = ModBlockEntityFields {
                items: vec![None; slots],
                ..Default::default()
            };
            let entity = ModBlockEntity::new("take:pot".into(), fields);
            game.world
                .block_entities
                .insert(POS, BlockEntity::Mod(Box::new(entity)));
            let host = game.lan.as_mut().unwrap();
            let title = sunrise_core::nbt::Tag::String("pot".into());
            let items = vec![None; slots];
            host.hub
                .open_container(12, MENU, title, POS, Slots::Block(POS), items, None);
            host.hub.guests.update(0, |guest| {
                guest.menu.as_mut().unwrap().mod_type = Some(menu_type);
            });
            FAIL.with(|fail| *fail.borrow_mut() = false);
            heard();
            test(game);
        });
    });
}

/// Sets the pot's slots (`pot`), the guest's inventory (`inventory`, by index) and cursor.
fn set(
    game: &mut crate::game::Game<'_>,
    pot: &[(usize, Option<ItemStack>)],
    inventory: &[(usize, Option<ItemStack>)],
    carried: Option<ItemStack>,
) {
    let slots = game.world.block_entities.get_mut(&POS).unwrap().slots_mut();
    slots.fill(None);
    for (slot, stack) in pot {
        slots[*slot] = stack.clone();
    }
    game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
        guest.inventory.fill(None);
        for (slot, stack) in inventory {
            guest.inventory[*slot] = stack.clone();
        }
        guest.carried = carried;
    });
}
/// The guest's click (`input` is `ClickType`'s id), claiming nothing.
fn click(game: &mut crate::game::Game<'_>, slot: i32, button: i8, input: i32) {
    let menu = game.lan.as_ref().unwrap().hub.guests[0].menu.as_ref();
    let (container, state) = menu.map(|m| (m.window, m.state)).unwrap();
    game.lan_guest_packet(
        12,
        Serverbound::ContainerClick(ContainerClick {
            container,
            state,
            slot: i16::try_from(slot).unwrap(),
            button,
            input,
            changed: Vec::new(),
            carried: None,
        }),
    );
}
/// The count in pot slot `slot`.
fn in_pot(game: &crate::game::Game<'_>, slot: usize) -> u32 {
    let slots = game.world.block_entities.get(&POS).unwrap().slots();
    slots[slot].as_ref().map_or(0, |s| s.count)
}
/// The count in the guest's inventory slot `slot`, and on their cursor.
fn held(game: &crate::game::Game<'_>, slot: usize) -> (u32, u32) {
    let guest = &game.lan.as_ref().unwrap().hub.guests[0];
    let count = |stack: &Option<ItemStack>| stack.as_ref().map_or(0, |s| s.count);
    (count(&guest.inventory[slot]), count(&guest.carried))
}

#[test]
#[ignore = "daily"]
fn a_pickup_hears_what_left_the_slot_and_what_stayed() {
    with_pot(pot_type(SLOTS, None), |game| {
        // The left button takes all five, the right button half rounded up.
        set(game, &[(0, stone(5))], &[], None);
        click(game, 0, 0, 0);
        assert_eq!(heard(), [took(0, 5, 0)]);
        set(game, &[(0, stone(5))], &[], None);
        click(game, 0, 1, 0);
        assert_eq!(heard(), [took(0, 3, 2)]);
        // The result slot refuses placement: a matching cursor takes all of it.
        set(game, &[(RESULT, stone(4))], &[], stone(3));
        click(game, RESULT as i32, 0, 0);
        assert_eq!(heard(), [took(RESULT, 4, 0)]);
        assert_eq!(held(game, 0).1, 7);
        // The hook is the server's: the client's prediction of the same take hears nothing.
        let mut menu = Menu::new(Kind::Mod(pot_type(SLOTS, None)));
        menu.set_registry_id(Some(MENU));
        menu.load_slot(0, stone(5));
        menu.click(&game.menu_env(), 0, 0, Input::Pickup).unwrap();
        assert_eq!(menu.carried.as_ref().map(|c| c.count), Some(5));
        assert!(heard().is_empty());
    });
}

#[test]
#[ignore = "daily"]
fn a_swap_hears_the_slots_stack() {
    with_pot(pot_type(SLOTS, None), |game| {
        // Into an empty hotbar slot: the whole stack. With dirt there: the slot's former stack,
        // the dirt now in the slot.
        set(game, &[(0, stone(5))], &[], None);
        click(game, 0, 0, 2);
        assert_eq!(heard(), [took(0, 5, 0)]);
        set(
            game,
            &[(0, stone(5))],
            &[(0, item("minecraft:dirt", 2))],
            None,
        );
        click(game, 0, 0, 2);
        let dirt = Some(("minecraft:dirt".to_owned(), 2));
        assert_eq!(
            heard(),
            [(12, MENU.into(), 0, ("minecraft:stone".into(), 5), dirt)]
        );
        // Into the empty off hand (button 40): the whole stack.
        set(game, &[(0, stone(5))], &[], None);
        click(game, 0, 40, 2);
        assert_eq!(heard(), [took(0, 5, 0)]);
    });
}

#[test]
#[ignore = "daily"]
fn a_throw_and_pick_all_hear_each_take() {
    with_pot(pot_type(SLOTS, None), |game| {
        // A throw of one, and of the stack (one take: the slot is empty after it).
        set(game, &[(0, stone(5))], &[], None);
        click(game, 0, 0, 4);
        assert_eq!(heard(), [took(0, 1, 4)]);
        set(game, &[(0, stone(5))], &[], None);
        click(game, 0, 1, 4);
        assert_eq!(heard(), [took(0, 5, 0)]);
        // Pick-all on an empty slot gathers from slots 0 and 3, each heard with its part, and
        // from the hotbar, which is the player's own and not heard.
        set(
            game,
            &[(0, stone(3)), (3, stone(2))],
            &[(4, stone(6))],
            stone(1),
        );
        click(game, 5, 0, 6);
        assert_eq!(heard(), [took(0, 3, 0), took(3, 2, 0)]);
        assert_eq!(held(game, 4), (0, 12));
    });
}

#[test]
#[ignore = "daily"]
fn a_shift_click_hears_the_amount_moved_once_per_craft() {
    with_pot(pot_type(SLOTS, None), |game| {
        // A plain slot: the stack moved to the inventory, once.
        set(game, &[(0, stone(5))], &[], None);
        click(game, 0, 0, 1);
        assert_eq!(heard(), [took(0, 5, 0)]);
        // The result slot holding many crafts' output (a furnace's shape): one move takes all
        // twenty, heard once.
        set(game, &[(RESULT, stone(20))], &[], None);
        click(game, RESULT as i32, 0, 1);
        assert_eq!(heard(), [took(RESULT, 20, 0)]);
        // Room for four only (the inventory full but for four stone of room): one notice with
        // the four moved and the sixteen left; with no room at all, nothing moves and nothing
        // is heard.
        let dirt = |slot| (slot, item("minecraft:dirt", 64));
        let mut full: Vec<_> = (1..36).map(dirt).collect();
        full.push((0, stone(60)));
        set(game, &[(RESULT, stone(20))], &full, None);
        click(game, RESULT as i32, 0, 1);
        assert_eq!(heard(), [took(RESULT, 4, 16)]);
        full.pop();
        full.push(dirt(0));
        set(game, &[(RESULT, stone(20))], &full, None);
        click(game, RESULT as i32, 0, 1);
        assert!(heard().is_empty());
        assert_eq!(in_pot(game, RESULT), 20);
    });
    // A crafting grid (slots 0 to 8) and its result (slot 9): three oak logs make planks three
    // times, each craft heard with its four planks.
    let grid = ModCrafting {
        grid: 0,
        size: 3,
        result: 9,
    };
    with_pot(pot_type(10, Some(grid)), |game| {
        let logs = item("minecraft:oak_log", 3);
        set(game, &[(0, logs.clone())], &[], None);
        game.lan.as_mut().unwrap().hub.guests.update(0, |guest| {
            let mut shown = vec![None; 9];
            shown[0] = logs;
            let result = item("minecraft:oak_planks", 4);
            guest.menu.as_mut().unwrap().craft = Some(ModCraft {
                grid: shown,
                result,
            });
        });
        click(game, 9, 0, 1);
        let crafts: Vec<_> = heard()
            .into_iter()
            .map(|(_, _, slot, taken, _)| (slot, taken))
            .collect();
        let planks = (9, ("minecraft:oak_planks".to_owned(), 4));
        assert_eq!(crafts, [planks.clone(), planks.clone(), planks]);
        assert_eq!(in_pot(game, 0), 0);
    });
}

#[test]
#[ignore = "daily"]
fn an_exchange_an_insertion_and_a_refused_take_are_silent() {
    with_pot(pot_type(SLOTS, None), |game| {
        // Dirt on the cursor over stone: the stacks change places, nothing is taken.
        set(game, &[(0, stone(5))], &[], item("minecraft:dirt", 2));
        click(game, 0, 0, 0);
        assert_eq!(in_pot(game, 0), 2);
        // Stone put into an empty slot.
        set(game, &[], &[], stone(5));
        click(game, 0, 0, 0);
        assert_eq!(in_pot(game, 0), 5);
        // The slot whose pickup rule refuses: nothing leaves it.
        set(game, &[(KEPT, stone(5))], &[], None);
        click(game, KEPT as i32, 0, 0);
        assert_eq!(in_pot(game, KEPT), 5);
        assert!(heard().is_empty());
    });
}

#[test]
#[ignore = "daily"]
fn a_failing_listener_undoes_its_writes_and_the_take() {
    with_pot(pot_type(SLOTS, None), |game| {
        // The listener puts one stone in hotbar slot 8, then fails: the stone it put is gone,
        // the five stone are back in the slot and the cursor is empty.
        FAIL.with(|fail| *fail.borrow_mut() = true);
        set(game, &[(0, stone(5))], &[], None);
        click(game, 0, 0, 0);
        assert_eq!(heard(), [took(0, 5, 0)]);
        assert_eq!(in_pot(game, 0), 5);
        assert_eq!(held(game, 8), (0, 0));
        // A listener that returns keeps its write, and the take stands.
        FAIL.with(|fail| *fail.borrow_mut() = false);
        click(game, 0, 0, 0);
        assert_eq!(heard(), [took(0, 5, 0)]);
        assert_eq!(in_pot(game, 0), 0);
        assert_eq!(held(game, 8), (1, 5));
    });
}
