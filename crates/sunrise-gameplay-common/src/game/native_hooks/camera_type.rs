use super::*;

#[cfg(test)]
pub mod camera_type_tests {
    use mod_api::{CameraType, CameraTypeCycleEvent, ClientHost, Hooks};
    use std::sync::{Arc, Mutex};

    /// Skips the front view: F5 goes first person, behind, first person.
    pub struct NoFront(
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
    pub struct Calls {
        pub(in crate::game) poses: std::sync::atomic::AtomicUsize,
        pub(in crate::game) cycles: std::sync::atomic::AtomicUsize,
    }
    pub struct SharedCamera(pub(in crate::game) Arc<Calls>);
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
