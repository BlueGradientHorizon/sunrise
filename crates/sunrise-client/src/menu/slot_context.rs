//! The staged slot view lent to a `menu_slot_rules` ask and the reads needed to replay its answer.
use super::{
    CONTAINER_MAX, Env, GRID, ItemStack, Kind, MAIN, Menu, PLAYER, RESULT, Role, Slot, SlotContext,
    SlotRules, item_max_stack, limit_slot_max_for, vanilla_may_place_for_role,
    vanilla_slot_max_for_role,
};
use mod_api::MenuContainer;
use std::cell::RefCell;

/// A slot rule's dependency on the planner's view. Store only what the rule read, including
/// empty slots; replaying the rule does not rebuild or convert the menu's slot matrix.
#[derive(Clone, Debug)]
pub(super) struct SlotReads {
    player: Option<u128>,
    pos: Option<[i32; 3]>,
    container: MenuContainer,
    slots: Vec<(MenuContainer, usize, Option<ItemStack>)>,
}

impl SlotReads {
    /// Compare through the view so an enclosing record depends on this comparison too. Even
    /// a replay miss depends on the slot that caused it: a later stage could match this answer.
    pub(super) fn matches(&self, context: SlotContext<'_>) -> bool {
        (self.player, self.pos, self.container) == (context.player, context.pos, context.container)
            && self
                .slots
                .iter()
                .all(|(container, index, stack)| context.slot(*container, *index) == stack.as_ref())
    }
}

/// The API's key for one of a menu's containers.
fn key(container: usize) -> MenuContainer {
    match container {
        MAIN => MenuContainer::Main,
        GRID => MenuContainer::Grid,
        RESULT => MenuContainer::Result,
        _ => MenuContainer::Player,
    }
}

/// The container a LAN view's slot `i` is in: the block's container's slots are `Main`, the
/// rest the player's.
pub fn lan_container(i: usize, container: Option<&[Option<ItemStack>]>) -> MenuContainer {
    match container {
        Some(c) if i < c.len() => MenuContainer::Main,
        _ => MenuContainer::Player,
    }
}

impl SlotContext<'_> {
    pub(super) fn record<R>(self, ask: impl FnOnce(SlotContext<'_>) -> R) -> (R, SlotReads) {
        let reads = RefCell::new(Vec::new());
        let trace = |container, index, stack: Option<&ItemStack>| {
            if let Some(outer) = self.trace {
                outer(container, index, stack);
            }
            let mut reads = reads.borrow_mut();
            if !reads.iter().any(|&(c, i, _)| (c, i) == (container, index)) {
                reads.push((container, index, stack.cloned()));
            }
        };
        let answer = ask(SlotContext {
            trace: Some(&trace),
            ..self
        });
        let reads = SlotReads {
            player: self.player,
            pos: self.pos,
            container: self.container,
            slots: reads.into_inner(),
        };
        (answer, reads)
    }

    /// What slot `index` of `container` holds, keyed as `MenuSlotContext::slot`.
    pub fn slot(&self, container: MenuContainer, index: usize) -> Option<&ItemStack> {
        let stack = match container {
            MenuContainer::Main => {
                if self.main_crafting.is_some_and(|crafting| {
                    crafting.grid_slots().contains(&index) || index == crafting.result
                }) {
                    None
                } else if index < self.slots.len() {
                    self.slots.get(index)
                } else {
                    self.slots_second.get(index - self.slots.len())
                }
            }
            MenuContainer::Grid => self.grid.get(index),
            MenuContainer::Result => self.result.get(index),
            _ => None,
        }
        .and_then(Option::as_ref);
        if let Some(trace) = self.trace {
            trace(container, index, stack);
        }
        stack
    }
}

impl Menu {
    /// Who and where this menu's `menu_slot_rules` ask about menu slot `s` is: the env's
    /// player, the menu's block (`set_block_pos`), `s`'s container and the menu's own
    /// staged containers.
    pub(super) fn slot_context<'s>(&'s self, env: &Env, s: usize) -> SlotContext<'s> {
        let container = self
            .state
            .slots
            .get(s)
            .map_or(PLAYER, |slot| slot.container);
        SlotContext {
            player: env.player,
            pos: self.state.block_pos,
            container: key(container),
            slots: &self.state.containers[MAIN],
            slots_second: &[],
            main_crafting: None,
            grid: &self.state.containers[GRID],
            result: &self.state.containers[RESULT],
            trace: None,
        }
    }
}

/// A resolved menu's borrowed slot state, for rule queries that do not edit the menu.
/// The caller validates the menu and any per-open slot exclusions, then lends the containers
/// and the queried slot's state.
pub struct LiveSlotRules<'a> {
    pub kind: Kind,
    pub registry_id: Option<&'a str>,
    pub pos: Option<[i32; 3]>,
    pub main: &'a [Option<ItemStack>],
    pub main_second: &'a [Option<ItemStack>],
    pub grid: &'a [Option<ItemStack>],
    pub result: &'a [Option<ItemStack>],
    pub container_limit: Option<u32>,
    pub crafter_disabled: bool,
    pub creative: bool,
}

impl<'a> LiveSlotRules<'a> {
    pub(super) fn slot_context(&self, env: &Env, slot: Slot) -> SlotContext<'a> {
        SlotContext {
            player: env.player,
            pos: self.pos,
            container: key(slot.container),
            slots: self.main,
            slots_second: self.main_second,
            main_crafting: match self.kind {
                Kind::Mod(t) => t.crafting,
                _ => None,
            },
            grid: self.grid,
            result: self.result,
            trace: None,
        }
    }

    pub(super) fn vanilla_answers(
        &self,
        env: &Env,
        slot: Slot,
        stack: &ItemStack,
    ) -> (bool, bool, u32) {
        (
            vanilla_may_place_for_role(
                slot.role,
                env,
                env.can_equip,
                stack,
                self.crafter_disabled,
                None,
            ),
            true,
            vanilla_slot_max_for_role(self.kind, slot, stack),
        )
    }

    /// The maximum count allowed by the live slot, or zero if it refuses placement.
    /// `None` means the index is not part of this menu layout.
    pub fn slot_holds(&self, env: &Env, s: usize, stack: &ItemStack) -> Option<u32> {
        let slot = self.kind.slot(s)?;
        if slot.role == Role::Hidden {
            return Some(0);
        }
        let own = self.vanilla_answers(env, slot, stack);
        let item_max = item_max_stack(env.items, stack);
        let (place, _, max) = if let Some(id) = self.registry_id {
            apply_menu_slot_rules(
                env.slot_rules,
                id,
                slot.index,
                stack,
                self.slot_context(env, slot),
                own,
                item_max,
            )
        } else {
            (own.0, true, own.2.min(item_max))
        };
        Some(if place {
            limit_slot_max_for(self.kind, s, slot, self.container_limit, max)
        } else {
            0
        })
    }

    /// Whether the live slot gives up its stack, or `None` for an index outside the layout.
    pub fn slot_gives(&self, env: &Env, s: usize, held: Option<&ItemStack>) -> Option<bool> {
        self.pickup_rules(env, s, held, None)
    }
}

/// Apply the scoped rule to a slot's own answers (`may_place`, `may_pickup` and the maximum, in
/// that order), then enforce its item cap. The rules are asked here, while the click plans, and
/// only for a menu type a part answers; with none lent the slot's answers stand.
pub fn apply_menu_slot_rules(
    rules: Option<&dyn SlotRules>,
    menu: &str,
    slot: usize,
    stack: &ItemStack,
    context: SlotContext<'_>,
    own: (bool, bool, u32),
    item_max: u32,
) -> (bool, bool, u32) {
    let (mut may_place, mut may_pickup, mut max_stack) = own;
    if let Some(rules) = rules.filter(|rules| rules.listens(menu)) {
        rules.rule(
            menu,
            slot,
            stack,
            context,
            &mut may_place,
            &mut may_pickup,
            &mut max_stack,
        );
    }
    let max_stack = max_stack.clamp(1, CONTAINER_MAX).min(item_max);
    (may_place, may_pickup, max_stack)
}
