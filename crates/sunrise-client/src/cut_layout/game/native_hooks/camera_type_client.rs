

#[cfg(test)]
pub(in crate::game) mod camera_type_tests {
    use sunrise_gameplay_common::game::native_hooks::camera_type::camera_type_tests::Calls;
    use sunrise_gameplay_common::game::native_hooks::camera_type::camera_type_tests::NoFront;
    use sunrise_gameplay_common::game::native_hooks::camera_type::camera_type_tests::SharedCamera;
    use mod_api::{CameraType, ModEntry};
    use mod_platform::testing::with_mods;
    use std::sync::{Arc, Mutex};

    /// `key.togglePerspective`: the mod hears each cycle with vanilla's next type and its
    /// answer is the new type; setting the type does not cycle, and without a listener F5
    /// cycles as vanilla does.
    #[test]
    #[ignore = "daily"]
    fn a_client_mod_picks_the_camera_type_f5_cycles_to() {
        sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
            let mut seen = Vec::new();
            for _ in 0..3 {
                game.cycle_camera();
                seen.push(game.camera_type());
            }
            use CameraType::*;
            assert_eq!(seen, [ThirdPersonBack, ThirdPersonFront, FirstPerson]);
        });
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mods: mod_api::Mods = vec![ModEntry::new(
            "no-front",
            Box::new(NoFront(Arc::clone(&heard))),
        )]
        .into();
        with_mods(mods, || {
            sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
                use CameraType::*;
                game.cycle_camera();
                assert_eq!(game.camera_type(), ThirdPersonBack);
                game.cycle_camera();
                assert_eq!(game.camera_type(), FirstPerson, "the front view is skipped");
                game.set_camera_type(ThirdPersonFront);
                assert_eq!(game.camera_type(), ThirdPersonFront);
                assert_eq!(game.side.camera_type, FirstPerson, "the request is pending");
                assert_eq!(heard.lock().unwrap().len(), 2, "setting is not a cycle");
                game.cycle_camera();
                assert_eq!(game.side.camera_type, FirstPerson, "F5's final choice wins");
            });
        });
        use CameraType::*;
        assert_eq!(
            *heard.lock().unwrap(),
            [
                (FirstPerson, ThirdPersonBack),
                (ThirdPersonBack, ThirdPersonFront),
                (ThirdPersonFront, FirstPerson)
            ],
            "a prior request changes the next F5's previous type without an extra cycle"
        );
    }

    #[test]
    #[ignore = "daily"]
    fn shared_camera_pose_requests_apply_before_frame_consumers() {
        use std::sync::atomic::Ordering;
        use sunrise_core::geometry::Vec3;
        use sunrise_sim::motion::PlayerMotion;

        let calls = Arc::new(Calls::default());
        let mods: mod_api::Mods = vec![ModEntry::new(
            "shared-camera",
            Box::new(SharedCamera(Arc::clone(&calls))),
        )]
        .into();
        with_mods(mods, || {
            sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
                game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
                game.side.title.0 = None;
                game.side.spectated_post_effect = Some("minecraft:creeper");
                let vanilla = game.camera_view(1.0).unwrap();
                assert_eq!(vanilla.0.z, 0.0);
                let posed = game.native_camera_pose_now(1.0, vanilla);
                assert_eq!(game.side.camera_type, CameraType::ThirdPersonBack);
                assert_eq!(game.camera_type(), CameraType::ThirdPersonBack);
                assert_eq!(game.side.spectated_post_effect, None);
                assert!((posed.0.z + 4.0).abs() < 1e-6, "refresh unchanged position");
                assert_eq!(posed.1, 15.0, "preserve the explicit yaw override");
                assert_eq!(posed.2, vanilla.2);
            });
        });
        assert_eq!(calls.poses.load(Ordering::Relaxed), 1);
        assert_eq!(calls.cycles.load(Ordering::Relaxed), 0);
    }
}