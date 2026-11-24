#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::camera_read::PROJECT_VIEW;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::camera_read::ProjectionFrame;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::camera_read::*;

/// Helpers for native mods, kept as stable as the hook points.
impl Game<'_> {
    /// The most recently extracted world frame's camera, if one exists.
    /// During GUI rendering this is the current frame, including bob/spin and FOV.
    pub(in crate::game) fn camera(&self) -> Option<mod_api::Camera> {
        self.client.native_camera_frame.clone()
    }
    /// The entity the view follows (`Minecraft.getCameraEntity`): the entity a server attached
    /// the camera to, else the local player. None outside a client world.
    pub(in crate::game) fn camera_entity(&self) -> Option<EntityRef> {
        if !self.native_window() || self.title.0.is_some() {
            return None;
        }
        self.player.as_ref()?;
        let id = self
            .remote_camera_id()
            .unwrap_or_else(|| self.own_entity_id() as u32);
        Some(EntityRef::new(self.native_lifecycle.epoch()?, id))
    }
    /// `GameRenderer.projectPointToScreen` against this frame's camera; None behind the camera,
    /// for a non-finite point, outside a world and before the first frame.
    pub(in crate::game) fn project(&self, at: [f64; 3]) -> Option<[f32; 3]> {
        if PROJECT_VIEW.get() != self as *const Self as usize {
            return None;
        }
        let camera = self.client.native_camera_frame.as_ref()?;
        sunrise_client::camera::project_point(
            &camera.projection,
            camera.position,
            camera.yaw,
            camera.pitch,
            at,
        )
    }
}
