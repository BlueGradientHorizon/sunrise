#[allow(unused_imports)]
use super::*;

thread_local! {
    pub(in crate::game) static PROJECT_VIEW: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
pub(in crate::game) struct ProjectionFrame(pub(in crate::game) usize);
impl Drop for ProjectionFrame {
    fn drop(&mut self) {
        PROJECT_VIEW.set(self.0);
    }
}

/// Helpers for native mods, kept as stable as the hook points.
impl Game<'_> {
    pub(in crate::game) fn activate_projection_frame(&self) -> ProjectionFrame {
        ProjectionFrame(PROJECT_VIEW.replace(self as *const Self as usize))
    }
    /// `Minecraft.setCameraEntity`: the view follows the local player or a mob the server shows
    /// this window. False, with the camera untouched, for anything else.
    pub(in crate::game) fn set_camera_entity(&mut self, entity: EntityRef) -> bool {
        use crate::game::commands::Target;
        let id = match self.native_client_entity_target(entity) {
            Some(Target::Player | Target::Mob(_)) => entity.id() as i32,
            _ => return false,
        };
        self.remote_set_camera(id);
        true
    }
    /// Collision boxes in block-local coordinates with an empty collision context.
    pub(in crate::game) fn block_collision(&self, pos: [i32; 3]) -> Vec<[f64; 6]> {
        self.native_block_collision_in(&self.world, pos)
    }
}
