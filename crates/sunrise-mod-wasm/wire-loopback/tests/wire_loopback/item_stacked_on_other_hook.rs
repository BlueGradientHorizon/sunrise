use super::*;

pub(super) struct ItemStackedOnOtherHooks;
impl ItemStackedOnOtherHooks {
    pub(super) fn item_stacked_on_other(
        &self,
        _: mod_api::HostSide,
        event: &mut mod_api::ItemStackedOnOtherEvent<'_>,
    ) -> mod_api::Flow {
        // A cancelled, handled click that moves the cursor's stack into the slot.
        use mod_api::item::menu::StackResult;
        event.result.handled = true;
        event.result.clicked = StackResult::Set(event.event.carried.clone());
        event.result.carried = StackResult::Set(None);
        event.cancelled = true;
        mod_api::Flow::Continue
    }
}

#[test]
fn wire_loopback_nested_tuple_reference_writes_back() {
    mod_api::__private::initialize().unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let module = WasmMod {
        subscribed: &[],
        listeners: &[],
        group: 0,
        corrupt: false,
    };
    install_counter();
    struct Client;
    impl mod_api::Host for Client {}
    mod_api::impl_host_areas!(Client);
    impl mod_api::ClientHost for Client {}
    mod_api::impl_client_host_areas!(Client);
    use mod_api::item::menu::{StackResult, StackedOnOther, StackedOnOtherResult};
    let stone = mod_api::ItemStack::new(REGISTERED_ITEM.get().unwrap(), 3).unwrap();
    let event = StackedOnOther::new(Some(7), Some(9), 2, 0, 2, true, Some(stone.clone()), None);
    let mut result = StackedOnOtherResult::default();
    let mut event = mod_api::ItemStackedOnOtherEvent {
        event: &event,
        result: &mut result,
        cancelled: false,
    };
    // The typed answer crosses the boundary both ways: the frozen stacks in, the stacks out.
    let flow = module.item_stacked_on_other(mod_api::HostSide::Client(&mut Client), &mut event);
    assert!(matches!(flow, mod_api::Flow::Continue));
    assert!(event.cancelled);
    assert!(result.handled);
    assert_eq!(result.carried, StackResult::Set(None));
    assert_eq!(result.clicked, StackResult::Set(Some(stone.clone())));
    // Side by side with the same listener run natively (no wire): the same outcome.
    struct Native;
    #[mod_api::__subscriptions]
    impl Hooks for Native {
        fn item_stacked_on_other(
            &self,
            _: mod_api::HostSide,
            event: &mut mod_api::ItemStackedOnOtherEvent<'_>,
        ) -> mod_api::Flow {
            event.result.handled = true;
            event.result.clicked = StackResult::Set(event.event.carried.clone());
            event.result.carried = StackResult::Set(None);
            event.cancelled = true;
            mod_api::Flow::Continue
        }
    }
    let frozen = StackedOnOther::new(Some(7), Some(9), 2, 0, 2, true, Some(stone), None);
    let mut direct = StackedOnOtherResult::default();
    let mut native_event = mod_api::ItemStackedOnOtherEvent {
        event: &frozen,
        result: &mut direct,
        cancelled: false,
    };
    let native_flow =
        Native.item_stacked_on_other(mod_api::HostSide::Client(&mut Client), &mut native_event);
    assert!(matches!(native_flow, mod_api::Flow::Continue));
    assert!(native_event.cancelled);
    assert_eq!(direct, result);
}
