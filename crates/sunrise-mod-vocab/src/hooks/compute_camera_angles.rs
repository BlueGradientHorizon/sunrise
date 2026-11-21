//! The `compute_camera_angles` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "ViewportEvent.ComputeCameraAngles",
        cancellable = false
    )]
    /// Client camera angles before `Camera.setRotation`, third-person mirroring, detached back-off,
    /// and sleeping bed rotation (`ViewportEvent.ComputeCameraAngles` in `Camera.alignWithEntity`).
    ///
    /// Starts from the camera entity's interpolated `(view_y_rot(partial), view_x_rot(partial), 0.0)`
    /// in degrees. Listeners compose in dependency order; any non-finite component falls back to
    /// vanilla's value for that component.
    ///
    /// Third-person front mirroring (`yaw + 180`, `-pitch`, `-roll`), detached back-off along the
    /// rotated look vector, and sleeping bed rotation run after this hook, matching NeoForge.
    /// `ClientHost::set_camera_type` called before or during this hook takes effect before that
    /// detached placement runs. Changes affect rendering only — the player's own rotation and aim
    /// are unchanged.
    fn compute_camera_angles(partial: f32, yaw: &mut f32, pitch: &mut f32, roll: &mut f32);
}
