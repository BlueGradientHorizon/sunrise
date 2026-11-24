#[allow(unused_imports)]
use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// `key.togglePerspective` (F5) cycles the camera type: `previous` is the type it
    /// leaves, and `next` comes in as `CameraType.cycle` and may be set to another.
    pub(in crate::game) fn native_camera_type_cycle(
        &mut self,
        previous: mod_api::CameraType,
        next: &mut mod_api::CameraType,
    ) {
        self.fire_camera_type_cycle(previous, next);
    }
}

#[cfg(test)]
pub(in crate::game) mod camera_type_tests {
    #[allow(unused_imports)]
    use mod_api::{CameraType, CameraTypeCycleEvent, ClientHost, Hooks, ModEntry};
    #[allow(unused_imports)]
    use mod_platform::testing::with_mods;
    #[allow(unused_imports)]
    use std::sync::{Arc, Mutex};

    /// Skips the front view: F5 goes first person, behind, first person.
    pub(in crate::game) struct NoFront(
        pub(in crate::game) Arc<Mutex<Vec<(CameraType, CameraType)>>>,
    );
    #[mod_api::__subscriptions]
    impl Hooks for NoFront {
        fn camera_type_cycle(
            &self,
            host: &mut dyn ClientHost,
            event: &mut CameraTypeCycleEvent<'_>,
        ) {
            self.0.lock().unwrap().push((event.previous, *event.next));
            host.set_camera_type(CameraType::ThirdPersonFront);
            if *event.next == CameraType::ThirdPersonFront {
                *event.next = CameraType::FirstPerson;
            }
        }
    }

    #[derive(Default)]
    pub(in crate::game) struct Calls {
        pub(in crate::game) poses: std::sync::atomic::AtomicUsize,
        pub(in crate::game) cycles: std::sync::atomic::AtomicUsize,
    }
    pub(in crate::game) struct SharedCamera(pub(in crate::game) Arc<Calls>);
    #[mod_api::__subscriptions]
    impl Hooks for SharedCamera {
        fn camera_pose(&self, host: &dyn ClientHost, event: &mut mod_api::CameraPoseEvent<'_>) {
            self.0
                .poses
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            host.set_camera_type(CameraType::ThirdPersonFront);
            assert_eq!(host.camera_type(), CameraType::ThirdPersonFront);
            host.set_camera_type(CameraType::ThirdPersonBack);
            assert_eq!(host.camera_type(), CameraType::ThirdPersonBack);
            event.yaw += 15.0;
        }
        fn camera_type_cycle(&self, _: &mut dyn ClientHost, _: &mut CameraTypeCycleEvent<'_>) {
            self.0
                .cycles
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
