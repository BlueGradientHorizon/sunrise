//! `item_picked_up_post` on whole and partial pickups through the game's own pickup
//! paths, the host's player (`tick_dropped_items`) and a LAN guest (`guest_pickups`),
//! with literal counts.
use sunrise_gameplay::game::native_entity_refs::tests::collect_all_entities;
use mod_api::{Hooks, ItemPickedUpEvent, ItemPickedUpPostEvent, ModEntry, ServerHost, TriState};
use mod_platform::testing::with_mods;
use std::sync::{Arc, Mutex};
use sunrise_items::items::ItemStack;

#[derive(Default)]
pub(in crate::game) struct State {
    /// `item_picked_up`'s answer.
    pub(in crate::game) answer: TriState,
    /// A count the notice sets as the remainder.
    pub(in crate::game) raise: Option<u32>,
    /// Whether the notice empties the remainder.
    pub(in crate::game) empty: bool,
    /// Whether the notice removes every entity it can.
    pub(in crate::game) kill: bool,
    /// The notices heard, in order.
    pub(in crate::game) notices: Vec<Notice>,
    pub(in crate::game) touched: Vec<(u32, Option<mod_api::ItemStack>)>,
    pub(in crate::game) posted: Vec<u32>,
}

/// One notice heard: player, original count, remainder count.
pub(crate) type Notice = (u128, u32, Option<u32>);

#[derive(Clone, Default)]
pub(in crate::game) struct Probe(pub(in crate::game) Arc<Mutex<State>>);
#[mod_api::__subscriptions]
impl Hooks for Probe {
    fn item_picked_up(&self, host: &mut dyn ServerHost, event: &mut ItemPickedUpEvent<'_>) {
        let read = event
            .item
            .with_stack(host, |stack| stack.project())
            .flatten();
        let mut state = self.0.lock().unwrap();
        state.touched.push((event.item.entity().id(), read));
        *event.can_pickup = state.answer;
    }
    fn item_picked_up_post(
        &self,
        host: &mut dyn ServerHost,
        event: &mut ItemPickedUpPostEvent<'_>,
    ) {
        self.0.lock().unwrap().posted.push(event.item.entity().id());
        self.heard(host, event.player, event.original, event.remaining);
    }
}

impl Probe {
    /// Records the notice, then makes the case's changes.
    pub(crate) fn heard(
        &self,
        host: &mut dyn ServerHost,
        player: u128,
        original: &mod_api::ItemStack,
        remaining: &mut Option<mod_api::ItemStack>,
    ) {
        let (raise, empty, kill) = {
            let mut state = self.0.lock().unwrap();
            state.notices.push((
                player,
                original.count(),
                remaining.as_ref().map(|rest| rest.count()),
            ));
            (state.raise, state.empty, state.kill)
        };
        if let Some(count) = raise {
            let mut rest = original.clone();
            rest.set_count(count).unwrap();
            *remaining = Some(rest);
        }
        if empty {
            *remaining = None;
        }
        if kill {
            // A command where one runs (a LAN host), else the entity handle's removal, which
            // the level sweeps out of its list later.
            let _ = host.commands().run_command("kill @e[type=minecraft:item]");
            let level = host.level("minecraft:overworld").unwrap();
            for entity in collect_all_entities(host, level).unwrap_or_default() {
                let _ = entity.discard(host);
            }
        }
    }
}

/// One pickup case: the listener's setup, the inventory's room for apples,
/// and the ground stack's count and pickup delay.
pub(crate) struct Case {
    pub answer: TriState,
    pub raise: Option<u32>,
    pub empty: bool,
    pub kill: bool,
    pub room: u32,
    pub(crate) count: u32,
    pub(crate) delay: u32,
}

pub(crate) const CASE: Case = Case {
    answer: TriState::Default,
    raise: None,
    empty: false,
    kill: false,
    room: 8,
    count: 8,
    delay: 0,
};

/// What a case left: the notices heard, the apples on the ground (`None`
/// when the entity is gone) and the apples in the inventory's first slot.
pub(crate) type Outcome = (Vec<Notice>, Option<u32>, u32);

pub(crate) fn apples(room: u32) -> Option<ItemStack> {
    Some(match room {
        0 => ItemStack::new("minecraft:stone", 64),
        room => ItemStack::new("minecraft:apple", 64 - room),
    })
    .filter(|stack| stack.count > 0)
}

pub(crate) fn clear_ground(game: &mut sunrise_gameplay::game::ServerGame<'_>) {
    while !game.dropped.is_empty() {
        game.remove_dropped(0);
    }
}

pub(crate) fn arm(probe: &Probe, case: &Case) {
    let mut state = probe.0.lock().unwrap();
    state.answer = case.answer;
    state.raise = case.raise;
    state.empty = case.empty;
    state.kill = case.kill;
    state.notices.clear();
}

pub(crate) fn spawn(
    game: &mut sunrise_gameplay::game::ServerGame<'_>,
    at: [f64; 3],
    case: &Case,
) -> u32 {
    let level = game.level(game.dim.id()).unwrap();
    game.level_spawn_item(
        level,
        at[0],
        at[1],
        at[2],
        ItemStack::new("minecraft:apple", case.count),
        case.delay,
    )
    .unwrap()
    .id()
}

pub(crate) fn ground(game: &sunrise_gameplay::game::ServerGame<'_>, id: u32) -> Option<u32> {
    game.dropped
        .iter()
        .find(|item| item.id == id)
        .map(|item| item.stack.count)
}

pub(crate) fn with_probe(probe: &Probe, test: impl FnOnce()) {
    let mods: mod_api::Mods = vec![ModEntry::new("pickup_post", Box::new(probe.clone()))].into();
    with_mods(mods, test);
}

/// The host's player picks up through the lifted pass.
pub(crate) fn host_case(
    game: &mut sunrise_gameplay::game::ServerGame<'_>,
    probe: &Probe,
    case: &Case,
) -> Outcome {
    clear_ground(game);
    game.inventory
        .fill(Some(ItemStack::new("minecraft:stone", 64)));
    game.offhand = Some(ItemStack::new("minecraft:stone", 64));
    game.inventory[0] = apples(case.room);
    arm(probe, case);
    let id = spawn(game, [1.5, 64.0, 1.5], case);
    let mut round = game.native_tick_round();
    game.tick_dropped_items(&mut round);
    game.native_removals_flush();
    let first = game.inventory[0].as_ref().map_or(0, |stack| {
        if &*stack.item == "minecraft:apple" {
            stack.count
        } else {
            0
        }
    });
    let notices = std::mem::take(&mut probe.0.lock().unwrap().notices);
    (notices, ground(game, id), first)
}

#[allow(unused_imports)]
pub(crate) use sunrise_client_gameplay::game::client_side::lan_actions_client::pickups::pickup_post_tests::host_animations;

pub(crate) fn host_stat(game: &sunrise_gameplay::game::ServerGame<'_>) -> i64 {
    game.stats
        .values
        .get("minecraft:picked_up")
        .and_then(|items| items.get("minecraft:apple"))
        .map_or(0, |&n| i64::from(n))
}

/// Guest 12 picks up next to itself.
pub(crate) fn guest_case(
    game: &mut sunrise_gameplay::game::ServerGame<'_>,
    probe: &Probe,
    case: &Case,
) -> Outcome {
    clear_ground(game);
    let room = case.room;
    game.side
        .lan
        .as_mut()
        .unwrap()
        .hub
        .update_guest(12, |guest| {
            guest
                .inventory
                .fill(Some(ItemStack::new("minecraft:stone", 64)));
            guest.offhand = Some(ItemStack::new("minecraft:stone", 64));
            guest.inventory[0] = apples(room);
        })
        .unwrap();
    arm(probe, case);
    let id = spawn(game, [3.0, 64.0, 5.0], case);
    let gi = game.guest_index(12).unwrap();
    game.guest_pickups(gi);
    let guest = game.side.lan.as_ref().unwrap().hub.guest(12).unwrap();
    let first = guest.inventory[0].as_ref().map_or(0, |stack| {
        if &*stack.item == "minecraft:apple" {
            stack.count
        } else {
            0
        }
    });
    let notices = std::mem::take(&mut probe.0.lock().unwrap().notices);
    (notices, ground(game, id), first)
}

#[test]
#[ignore = "daily"]
fn guest_pickup_tells_whole_and_partial_pickups_and_keeps_listener_changes() {
    let probe = Probe::default();
    with_probe(&probe, || {
        sunrise_gameplay::game::native_guest_turn_tests::with_guests(|game| {
            // The whole stack goes in: one post notice naming guest 12, original 8.
            assert_eq!(
                guest_case(game, &probe, &CASE),
                (vec![(12, 8, None)], None, 64)
            );

            // 8 offered, 5 fit: one post notice, original 8 (untouched by the insertion),
            // remainder 3.
            let partial = Case { room: 5, ..CASE };
            assert_eq!(
                guest_case(game, &probe, &partial),
                (vec![(12, 8, Some(3))], Some(3), 64)
            );

            // The listener on a partial pickup raises the remainder to 6: the entity keeps 6.
            let raised = Case {
                raise: Some(6),
                ..partial
            };
            assert_eq!(
                guest_case(game, &probe, &raised),
                (vec![(12, 8, Some(3))], Some(6), 64)
            );

            // The listener on a partial pickup empties the remainder: the entity goes.
            let emptied = Case {
                empty: true,
                ..partial
            };
            assert_eq!(
                guest_case(game, &probe, &emptied),
                (vec![(12, 8, Some(3))], None, 64)
            );

            // The listener on a partial pickup kills the entity: it stays gone, with no panic.
            let killed = Case {
                kill: true,
                ..partial
            };
            assert_eq!(
                guest_case(game, &probe, &killed),
                (vec![(12, 8, Some(3))], None, 64)
            );

            // Refused by `item_picked_up`: nothing is told.
            let refused = Case {
                answer: TriState::False,
                ..CASE
            };
            assert_eq!(guest_case(game, &probe, &refused), (vec![], Some(8), 56));

            // A full inventory takes nothing, even when `True` forces the pickup.
            let full = Case { room: 0, ..CASE };
            assert_eq!(guest_case(game, &probe, &full), (vec![], Some(8), 0));
            let forced = Case {
                answer: TriState::True,
                room: 0,
                delay: 40,
                ..CASE
            };
            assert_eq!(guest_case(game, &probe, &forced), (vec![], Some(8), 0));

            // The listener puts 3 back: the entity stays with 3.
            let raised = Case {
                raise: Some(3),
                ..CASE
            };
            assert_eq!(
                guest_case(game, &probe, &raised),
                (vec![(12, 8, None)], Some(3), 64)
            );

            // The listener puts 3 back, then kills the entity: it stays gone, with no panic.
            let killed = Case {
                raise: Some(3),
                kill: true,
                ..CASE
            };
            assert_eq!(
                guest_case(game, &probe, &killed),
                (vec![(12, 8, None)], None, 64)
            );
        });
    });
}

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_ticks::NativeTicksGameExt1 as _;
