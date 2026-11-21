//! `menu_slot_rules` on the game's side. A menu click cannot reach the game, so the game lends
//! itself to the click as its [`SlotRules`] (`Env::slot_rules`), with the read-only `Host`
//! behind it: the click asks while it plans, once for each slot and stack it consults, and the
//! answers stay in what it planned. A menu type no part answers is never asked, and nothing is
//! converted for it.
use super::*;
use crate::game::api_values::TryIntoApi;
use std::cell::RefCell;
use sunrise_client::menu::{SlotContext, SlotRules};

thread_local! {
    static ASKING_SLOT_RULES: RefCell<()> = const { RefCell::new(()) };
}

impl SlotRules for Game<'_> {
    fn listens(&self, menu: &str) -> bool {
        mod_api::__private::has_part_hook(mod_api::HookId::MenuSlotRules)
            && mod_api::__private::menu_listens(mod_api::HookId::MenuSlotRules, menu)
    }

    fn rule(
        &self,
        menu: &str,
        slot: usize,
        stack: &ItemStack,
        context: SlotContext<'_>,
        may_place: &mut bool,
        may_pickup: &mut bool,
        max_stack: &mut u32,
    ) {
        ASKING_SLOT_RULES.with(|active| {
            // A Part can read a fresh MenuView. Skip nested dispatch, even through a different
            // slot or menu, and keep the slot's own answers. The borrow ends with this query.
            let Ok(_asking) = active.try_borrow_mut() else {
                return;
            };
            // A stack that cannot cross to the API is not asked about: the slot's answers stand.
            let Ok(stack) = stack.try_into_api() else {
                return;
            };
            // The menu's own containers, read only, keyed by `MenuContainer`; a stack that cannot
            // cross shows empty. Built once per click plan (the context's snapshot), and only here,
            // where a part listens.
            let build = || {
                let copy = |slots: &[Option<ItemStack>]| -> Vec<Option<mod_api::ItemStack>> {
                    slots
                        .iter()
                        .map(|slot| slot.as_ref().and_then(|stack| stack.try_into_api().ok()))
                        .collect()
                };
                let mut main = copy(context.slots);
                if let Some(crafting) = context.main_crafting {
                    for (index, slot) in main.iter_mut().enumerate() {
                        if crafting.grid_slots().contains(&index) || index == crafting.result {
                            *slot = None;
                        }
                    }
                }
                let slots = vec![main, copy(context.grid), copy(context.result)];
                mod_api::MenuSlotContext::new(context.player, context.pos, slots)
            };
            // A plan's copy made for another player or block (an env changed without a mutation)
            // is not theirs: that ask copies its own.
            let owned;
            let snapshot = match context.snapshot.map(|cell| cell.get_or_init(build)) {
                Some(shared) if (shared.player, shared.pos) == (context.player, context.pos) => {
                    shared
                }
                _ => {
                    owned = build();
                    &owned
                }
            };
            // Generated dispatch discards only the failing Part's edits to these answers.
            self.native_menu_slot_rules(mod_api::__private::MenuSlotRulesCall {
                menu,
                slot,
                container: context.container,
                stack: &stack,
                context: snapshot,
                may_place,
                may_pickup,
                max_stack,
            });
        });
    }
}
