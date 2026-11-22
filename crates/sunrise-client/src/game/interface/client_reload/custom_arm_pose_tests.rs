use crate::game::native_entity_refs::tests::{run_fresh_cases, with_client_game};
use mod_api::{ArmPose, ClientHost, CustomArmPose, Hand, Hooks, ModEntry};
use std::{collections::HashMap, sync::OnceLock};
use sunrise_client::mobs::{
    Mob,
    anim::{self, Poses},
};
use sunrise_client::{items::ItemStack, motion::PlayerMotion};
use sunrise_core::geometry::Vec3;
use sunrise_items::equipment::MAINHAND;

struct Skillet(OnceLock<CustomArmPose>);
#[mod_api::__subscriptions]
impl Hooks for Skillet {
    fn register(&self, registrar: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        self.0
            .set(registrar.register_arm_pose("skillet:flip", false, false))
            .unwrap();
    }
    fn get_arm_pose(
        &self,
        _: &dyn ClientHost,
        _: &mut mod_api::GetArmPoseEvent<'_>,
    ) -> Option<ArmPose> {
        Some(ArmPose::Custom(*self.0.get().unwrap()))
    }
    fn arm_pose_transform(
        &self,
        _: &dyn ClientHost,
        event: &mut mod_api::ArmPoseTransformEvent<'_>,
    ) {
        assert_eq!(event.pose, *self.0.get().unwrap());
        assert!(event.humanoid.main_hand.is_some());
        if event.right_arm {
            event.angles.right = [-0.71, 0.42, 0.63];
        } else {
            event.angles.left = [-0.71, 0.42, 0.63];
        }
    }
}
fn arms(mob: &Mob) -> [[f32; 9]; 2] {
    let mut poses = Poses::new(&mob.kind.model);
    anim::setup(mob, &anim::state(mob, 0.0), &mut poses, &HashMap::new());
    [*poses.get("right_arm"), *poses.get("left_arm")]
}

#[test]
#[ignore = "daily"]
fn registered_angles_reach_local_remote_and_humanoid_before_species_animation() {
    with_client_game(|game| {
        game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
        game.inventory[game.selected] = Some(ItemStack::new("minecraft:stick", 1));
        game.tick_player_model();
        for (id, kind) in [
            (60, game.res.player_type().clone().unwrap()),
            (
                61,
                game.res
                    .mob_types()
                    .get("minecraft:skeleton")
                    .unwrap()
                    .clone(),
            ),
            (
                62,
                game.res
                    .mob_types()
                    .get("minecraft:zombie")
                    .unwrap()
                    .clone(),
            ),
        ] {
            let mut mob = Mob::new(id, kind, Vec3::ZERO, 0.0);
            mob.ai.aggressive = false;
            mob.extra.main_hand_left = false;
            mob.extra.left_handed = false;
            mob.extra
                .set_slot_stack(MAINHAND, Some(ItemStack::new("minecraft:stick", 1)));
            game.mobs.push(mob);
        }
        let baseline: Vec<_> = game.mobs.iter().map(arms).collect();
        mod_platform::testing::with_mods(
            vec![ModEntry::new("skillet", Box::new(Skillet(OnceLock::new())))].into(),
            || {
                game.tick_remote_arm_poses();
                game.tick_player_model();
                for mob in [
                    game.player_model.as_ref().unwrap(),
                    &game.mobs[0],
                    &game.mobs[1],
                ] {
                    let posed = arms(mob);
                    let age = anim::state(mob, 0.0).age;
                    // Vanilla's arm bobbing follows the custom pose at the model's age.
                    // Mth's 65536-entry trig table differs from sin/cos by at most
                    // one sample; after the 0.05 scale that is below 0.000005.
                    let bob_x = (age * 0.067).sin() * 0.05;
                    let bob_z = (age * 0.09).cos() * 0.05 + 0.05;
                    assert!(
                        (posed[0][3] - (-0.71 + bob_x)).abs() < 0.000005,
                        "id {} main left {} age {age} poses {:?} arms {:?}",
                        mob.id,
                        mob.main_arm_left(),
                        mob.extra.held_arm_pose,
                        posed
                    );
                    assert!((posed[0][4] - 0.42).abs() < 1e-6);
                    assert!((posed[0][5] - (0.63 + bob_z)).abs() < 0.000005);
                }
                // Zombie animation is later than the base pose and overrides its arms.
                assert_eq!(arms(&game.mobs[2]), baseline[2]);
            },
        );
        game.tick_remote_arm_poses();
        assert_eq!(game.mobs.iter().map(arms).collect::<Vec<_>>(), baseline);
    });
}

#[test]
#[ignore = "daily"]
fn held_arm_pose_reads_the_rendered_humanoids_active_swing() {
    use std::sync::{Arc, Mutex};
    use sunrise_client::{first_person::SwingAnimation, mobs::ai::tick_swing};
    use sunrise_core::arm_pose::ArmPose::CrossbowHold;
    use sunrise_items::equipment::OFFHAND;

    if run_fresh_cases(
        "game::client_reload::tests::custom_arm_pose_tests::held_arm_pose_reads_the_rendered_humanoids_active_swing",
        "SUNRISE_ARM_POSE_SWING_CASE",
        &["humanoids"],
    ) {
        return;
    }

    type Sample = (u32, Hand, bool);
    struct IdlePose {
        item: mod_api::ItemKey,
        calls: Arc<Mutex<Vec<Sample>>>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for IdlePose {
        fn get_arm_pose(
            &self,
            _: &dyn ClientHost,
            event: &mut mod_api::GetArmPoseEvent<'_>,
        ) -> Option<ArmPose> {
            let swinging = event.humanoid.is_swinging;
            self.calls
                .lock()
                .unwrap()
                .push((event.entity_id, event.hand, swinging));
            (event.stack.item() == self.item && !swinging).then_some(ArmPose::CrossbowHold)
        }
    }

    with_client_game(|game| {
        let mut player = PlayerMotion::new(game.motion, Vec3::ZERO).unwrap();
        player.entity_id = 7;
        game.player = Some(player);
        game.inventory[game.selected] = Some(ItemStack::new("minecraft:stick", 1));
        game.offhand = Some(ItemStack::new("minecraft:brush", 1));

        for (id, kind) in [
            (60, game.res.player_type().clone().unwrap()),
            (
                61,
                game.res
                    .mob_types()
                    .get("minecraft:skeleton")
                    .unwrap()
                    .clone(),
            ),
        ] {
            let mut mob = Mob::new(id, kind, Vec3::ZERO, 0.0);
            mob.extra.main_hand_left = true;
            mob.extra
                .set_slot_stack(MAINHAND, Some(ItemStack::new("minecraft:brush", 1)));
            mob.extra
                .set_slot_stack(OFFHAND, Some(ItemStack::new("minecraft:stick", 1)));
            game.mobs.push(mob);
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let idle_pose = IdlePose {
            item: mod_api::__private::item_key("minecraft:stick").unwrap(),
            calls: calls.clone(),
        };
        mod_platform::testing::with_mods(
            vec![ModEntry::new("idle-pose", Box::new(idle_pose))].into(),
            || {
                let sample = |game: &mut crate::game::Game<'_>, active: [bool; 3]| {
                    game.tick_player_model();
                    game.tick_remote_arm_poses();
                    let expected = [7, 60, 61]
                        .into_iter()
                        .zip(active)
                        .flat_map(|(id, active)| Hand::BOTH.map(|hand| (id, hand, active)))
                        .collect::<Vec<_>>();
                    assert_eq!(std::mem::take(&mut *calls.lock().unwrap()), expected);
                    let hold = active.map(|swinging| (!swinging).then_some(CrossbowHold));
                    assert_eq!(
                        game.player_model.as_ref().unwrap().extra.held_arm_pose,
                        [hold[0], None]
                    );
                    assert_eq!(game.mobs[0].extra.held_arm_pose, [None, hold[1]]);
                    assert_eq!(game.mobs[1].extra.held_arm_pose, [None, hold[2]]);
                };

                let animation = SwingAnimation::DEFAULT;
                sample(game, [false; 3]);
                game.swing_hand(Hand::Off, animation);
                assert_eq!(game.swing.animation, 0.0);
                sample(game, [true, false, false]);
                for _ in 0..animation.duration + 2 {
                    game.swing.tick();
                }
                sample(game, [false; 3]);

                assert!(game.mobs[0].swing_hand_modified(false, animation));
                sample(game, [false, true, false]);
                // Mob swing time starts at -1; the first tick samples zero.
                for _ in 0..=animation.duration {
                    tick_swing(&mut game.mobs[0]);
                }
                sample(game, [false; 3]);

                game.mobs[1].swing();
                assert_eq!(game.mobs[1].extra.swing_right_arm, None);
                sample(game, [false, false, true]);
                for _ in 0..=game.mobs[1].swing_duration() {
                    tick_swing(&mut game.mobs[1]);
                }
                sample(game, [false; 3]);
            },
        );
    });
}

#[test]
#[ignore = "daily"]
fn suppressed_custom_offhand_never_calls_its_transformer() {
    use std::sync::{Arc, Mutex};
    struct PosesMod {
        main: OnceLock<CustomArmPose>,
        off: OnceLock<CustomArmPose>,
        calls: Arc<Mutex<Vec<bool>>>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for PosesMod {
        fn register(
            &self,
            registrar: &mut dyn mod_api::Registrar,
            _: &mod_api::RegistrationContext,
        ) {
            self.main
                .set(registrar.register_arm_pose("skillet:main", false, true))
                .unwrap();
            self.off
                .set(registrar.register_arm_pose("skillet:off", false, false))
                .unwrap();
        }
        fn get_arm_pose(
            &self,
            _: &dyn ClientHost,
            event: &mut mod_api::GetArmPoseEvent<'_>,
        ) -> Option<ArmPose> {
            Some(ArmPose::Custom(
                *if event.hand == mod_api::Hand::Main {
                    self.main.get()
                } else {
                    self.off.get()
                }
                .unwrap(),
            ))
        }
        fn arm_pose_transform(
            &self,
            _: &dyn ClientHost,
            event: &mut mod_api::ArmPoseTransformEvent<'_>,
        ) {
            assert_eq!(event.pose, *self.main.get().unwrap());
            assert_eq!(event.humanoid.main_arm_pose, ArmPose::Custom(event.pose));
            assert_eq!(event.humanoid.off_arm_pose, ArmPose::Empty);
            self.calls.lock().unwrap().push(event.right_arm);
        }
    }
    with_client_game(|game| {
        game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
        game.inventory[game.selected] = Some(ItemStack::new("minecraft:stick", 1));
        game.offhand = Some(ItemStack::new("minecraft:stick", 1));
        let calls = Arc::new(Mutex::new(Vec::new()));
        mod_platform::testing::with_mods(
            vec![ModEntry::new(
                "skillet",
                Box::new(PosesMod {
                    main: OnceLock::new(),
                    off: OnceLock::new(),
                    calls: calls.clone(),
                }),
            )]
            .into(),
            || {
                game.tick_player_model();
                assert_eq!(*calls.lock().unwrap(), [true]);
            },
        );
    });
}
