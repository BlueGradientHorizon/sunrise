use super::*;

/// The hook points. Each does nothing in Sunrise.
impl crate::game::ClientGame<'_> {
    /// The window's camera type (`Options.getCameraType`).
    pub(in crate::game) fn camera_type(&self) -> CameraType {
        self.client
            .requested_camera_type
            .get()
            .unwrap_or(self.client.camera_type)
    }
    /// Queue the window's camera type without cycling, including from shared hooks.
    pub(in crate::game) fn set_camera_type(&self, camera_type: CameraType) {
        self.client.requested_camera_type.set(Some(camera_type));
    }
}
