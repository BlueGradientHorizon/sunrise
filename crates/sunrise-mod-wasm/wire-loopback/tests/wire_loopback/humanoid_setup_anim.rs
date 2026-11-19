use super::*;

pub(super) struct HumanoidSetupHooks;
impl HumanoidSetupHooks {
    pub(super) fn humanoid_setup_anim(
        &self,
        _: &dyn mod_api::ClientHost,
        event: &mut mod_api::HumanoidSetupAnimEvent<'_>,
    ) {
        assert_eq!(event.entity_id, 73);
        assert_eq!(event.entity_type, "minecraft:player");
        assert_eq!(event.partial_tick, 0.25);
        assert_eq!(event.game_time, Some(123));
        assert!(event.humanoid.is_using_item);
        assert!(event.humanoid.main_arm_left);
        assert_eq!(event.humanoid.ticks_using_item, 4);
        assert_eq!(event.humanoid.main_hand.unwrap().count(), 3);
        assert_eq!(event.humanoid.use_hand, Some(mod_api::Hand::Main));
        assert!(
            !event
                .parts
                .set_rotation(mod_api::HumanoidPart::LeftArm, [f32::NAN, -0.4, 0.2])
        );
    }
}

#[test]
fn humanoid_setup_anim_native_and_wire_keep_context_and_refused_components() {
    struct Client;
    impl mod_api::Host for Client {}
    mod_api::impl_host_areas!(Client);
    impl mod_api::ClientHost for Client {}
    mod_api::impl_client_host_areas!(Client);
    install_counter();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let stack = mod_api::ItemStack::new(REGISTERED_ITEM.get().unwrap(), 3).unwrap();
    let humanoid = mod_api::HumanoidRenderState {
        main_hand: Some(&stack),
        off_hand: None,
        main_arm_left: true,
        is_using_item: true,
        use_hand: Some(mod_api::Hand::Main),
        ticks_using_item: 4,
        use_animation: mod_api::UseAnimation::None,
        main_arm_pose: mod_api::ArmPose::Item,
        off_arm_pose: mod_api::ArmPose::Empty,
    };
    let mut native =
        mod_api::HumanoidParts::new([Some([[1.0, 2.0, 3.0], [0.1, 0.2, 0.3]]); 7]).unwrap();
    let mut wire = native;
    for (parts, remote) in [(&mut native, false), (&mut wire, true)] {
        let mut event = mod_api::HumanoidSetupAnimEvent {
            entity_id: 73,
            entity_type: "minecraft:player",
            humanoid: &humanoid,
            partial_tick: 0.25,
            game_time: Some(123),
            parts,
        };
        if remote {
            WasmMod {
                group: 0,
                subscribed: &[],
                listeners: &[],
                corrupt: false,
            }
            .humanoid_setup_anim(&Client, &mut event);
        } else {
            HumanoidSetupHooks.humanoid_setup_anim(&Client, &mut event);
        }
    }
    assert_eq!(wire, native);
    assert_eq!(
        wire.get(mod_api::HumanoidPart::LeftArm).unwrap(),
        [[1.0, 2.0, 3.0], [0.1, -0.4, 0.2]]
    );
}
