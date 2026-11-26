//! `menu_slot_rules` on the game's side. A menu click cannot reach the game, so the game lends
//! itself to the click as its [`SlotRules`] (`Env::slot_rules`), with the read-only `Host`
//! behind it: each live slot check asks the parts directly. A menu type no part answers is never
//! asked, and nothing is converted for it.
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
            // A Part can query another slot or menu through its Host. Leave the slot's own
            // answers in place rather than dispatching menu Parts again on that same thread.
            let Ok(_asking) = active.try_borrow_mut() else {
                return;
            };
            // A stack that cannot cross to the API is not asked about: the slot's answers stand.
            let Ok(stack) = stack.try_into_api() else {
                return;
            };
            // Lend the live menu and player slots. Convert only what the part reads; a stack that
            // cannot cross the API boundary shows empty, as an absent slot does.
            let read = |container, index| context.slot(container, index)?.try_into_api().ok();
            let view = mod_api::MenuSlotContext::new(context.player, context.pos, &read);
            // The shared dispatcher discards only a failed callback's answers, reports the
            // failure, and continues the live slot check with the other parts.
            self.native_menu_slot_rules(mod_api::__private::MenuSlotRulesCall {
                menu,
                slot,
                container: context.container,
                stack: &stack,
                context: &view,
                may_place,
                may_pickup,
                max_stack,
            });
        });
    }

    fn pick_all_listens(&self, menu: &str) -> bool {
        mod_api::__private::has_part_hook(mod_api::HookId::MenuPickAll)
            && mod_api::__private::menu_listens(mod_api::HookId::MenuPickAll, menu)
    }

    fn pick_all(
        &self,
        menu: &str,
        slot: usize,
        carried: &ItemStack,
        stack: &ItemStack,
        context: SlotContext<'_>,
        allowed: &mut bool,
    ) {
        ASKING_SLOT_RULES.with(|active| {
            // As for slot rules: a Part's nested menu read answers without asking Parts again.
            let Ok(_asking) = active.try_borrow_mut() else {
                return;
            };
            // A stack that cannot cross to the API is not asked about: the gather takes it.
            let (Ok(carried), Ok(stack)) = (carried.try_into_api(), stack.try_into_api()) else {
                return;
            };
            let read = |container, index| context.slot(container, index)?.try_into_api().ok();
            let view = mod_api::MenuSlotContext::new(context.player, context.pos, &read);
            self.native_menu_pick_all(mod_api::__private::MenuPickAllCall {
                menu,
                slot,
                container: context.container,
                carried: &carried,
                stack: &stack,
                context: &view,
                allowed,
            });
        });
    }
}
