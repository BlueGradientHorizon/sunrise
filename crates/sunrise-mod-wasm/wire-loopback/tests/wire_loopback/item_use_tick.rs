use super::*;
pub(super) struct ItemUseTickHooks;
impl ItemUseTickHooks {
    pub(super) fn living_entity_use_item_tick(
        &self,
        _: mod_api::HostSide,
        event: &mut mod_api::LivingEntityUseItemTickEvent<'_>,
    ) -> mod_api::Flow {
        // The input names independent boundary cases, not an implementation
        // of the engine countdown: that behavior is tested in the game.
        match event.duration {
            32 => mod_api::Flow::Cancel,
            31 => {
                event.duration = 0;
                mod_api::Flow::Continue
            }
            30 => {
                event.duration = 1;
                mod_api::Flow::Continue
            }
            29 => {
                event.duration = -7;
                mod_api::Flow::Continue
            }
            _ => mod_api::Flow::Continue,
        }
    }
}

#[test]
fn item_use_tick_cancel_and_duration_replacements_cross_the_guest_on_both_sides() {
    struct World;
    impl mod_api::Host for World {}
    mod_api::impl_host_areas!(World);
    impl mod_api::ServerHost for World {}
    mod_api::impl_server_host_areas!(World);
    impl mod_api::ClientHost for World {}
    mod_api::impl_client_host_areas!(World);
    install_counter();
    let stack = mod_api::ItemStack::new(REGISTERED_ITEM.get().unwrap(), 1).unwrap();
    // with_mods replays registration; the shared guest registers wire_test arm poses.
    let mods = vec![mod_api::ModEntry::new(
        "wire_test",
        Box::new(WasmMod {
            group: 0,
            subscribed: &[Some(mod_api::HookId::LivingEntityUseItemTick)],
            listeners: &[],
            corrupt: false,
        }),
    )];
    mod_api::testing::with_mods(mods.into(), || {
        for client in [false, true] {
            for hand in [mod_api::Hand::Main, mod_api::Hand::Off] {
                for (input, expected, cancelled) in [
                    (32, 32, true),
                    (31, 0, false),
                    (30, 1, false),
                    (29, -7, false),
                    (28, 28, false),
                ] {
                    let (mut duration, mut canceled) = (input, false);
                    let mut world = World;
                    mod_api::dispatch_living_entity_use_item_tick(
                        if client {
                            mod_api::HostSide::Client(&mut world)
                        } else {
                            mod_api::HostSide::Server(&mut world)
                        },
                        &stack,
                        None,
                        hand,
                        &mut duration,
                        &mut canceled,
                    );
                    assert_eq!((duration, canceled), (expected, cancelled));
                }
            }
        }
    });
}
