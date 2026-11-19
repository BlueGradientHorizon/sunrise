//! `menu_slot_rules` through the generated host/guest dispatch against the same part called
//! natively: the answers, and how often the part is asked, are the same.
use super::*;
use mod_api::{ItemStack, MenuContainer, MenuPart, MenuSlotContext, MenuSlotRulesEvent};
use std::sync::atomic::{AtomicUsize, Ordering};
struct World;
impl mod_api::Host for World {}
mod_api::impl_host_areas!(World);
struct Part(Arc<AtomicUsize>);
#[mod_api::sunrise_part]
impl MenuPart for Part {
    fn menu_slot_rules(&self, _: &dyn mod_api::Host, event: &mut MenuSlotRulesEvent<'_>) {
        self.0.fetch_add(1, Ordering::Relaxed);
        if event.menu == "test:menu" && event.slot != 1 {
            event.may_place &= event.slot != 0;
            event.may_pickup &= event.slot != 2;
            event.max_stack = event.max_stack.min(3 + event.slot as u32);
        }
    }
}
#[test]
fn menu_slot_pickup_answers_and_asks_match_native_through_the_wire() {
    install_counter();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let wasm_asks = Arc::new(AtomicUsize::new(0));
    let proxy = WasmHandler {
        index: guest::add_handler(vec![guest::view::<dyn MenuPart>(Arc::new(Part(
            wasm_asks.clone(),
        )))]),
        module: WasmMod {
            group: 0,
            subscribed: &[],
            listeners: &[],
            corrupt: false,
        },
    };
    let native_asks = Arc::new(AtomicUsize::new(0));
    let native = Part(native_asks.clone());
    let stack = ItemStack::new(super::prepared_equip::helmet(), 1).unwrap();
    let mut answers = Vec::new();
    for menu in ["test:menu", "test:other"] {
        for slot in 0..3 {
            for (may_place, may_pickup, max_stack) in
                [(true, true, 64), (false, true, 1), (true, false, 2)]
            {
                let ask = |part: &dyn MenuPart| {
                    let mut event = MenuSlotRulesEvent {
                        menu,
                        slot,
                        container: MenuContainer::Main,
                        stack: &stack,
                        context: &MenuSlotContext::default(),
                        may_place,
                        may_pickup,
                        max_stack,
                    };
                    part.menu_slot_rules(&World, &mut event);
                    (event.may_place, event.may_pickup, event.max_stack)
                };
                let (wasm, direct) = (ask(&proxy), ask(&native));
                assert_eq!(
                    wasm, direct,
                    "{menu} {slot} {may_place} {may_pickup} {max_stack}"
                );
                answers.push(direct);
            }
        }
    }
    // Slot 0 of the menu the part answers for refuses placement and caps at 3, slot 2 refuses
    // pickup and caps at 5, slot 1 and the other menu are left as asked.
    assert_eq!(
        answers[..9],
        [
            (false, true, 3),
            (false, true, 1),
            (false, false, 2),
            (true, true, 64),
            (false, true, 1),
            (true, false, 2),
            (true, false, 5),
            (false, false, 1),
            (true, false, 2),
        ]
    );
    let asked = [(true, true, 64), (false, true, 1), (true, false, 2)];
    assert_eq!(answers[9..], asked.repeat(3));
    assert_eq!(wasm_asks.load(Ordering::Relaxed), 18);
    assert_eq!(native_asks.load(Ordering::Relaxed), 18);
}
