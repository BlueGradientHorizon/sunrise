//! The `camera_position` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "none (vanilla Camera.setPosition; NeoForge has no camera position event)",
        cancellable = false
    )]
    /// Render-only world-space camera position after `Camera.alignWithEntity` places the camera
    /// (`Camera.setPosition`).
    ///
    /// Starts from vanilla's placed world position in blocks (after entity interpolation, eye height,
    /// sleeping offset, and third-person collision back-off). Listeners compose in dependency order;
    /// any non-finite coordinate falls back to vanilla's value for that axis.
    ///
    /// Only the rendered camera position moves — the local player entity does not move, and
    /// eye/audio consumers that read the player's entity position continue to follow the player
    /// unless they read the camera itself (`ClientHost::camera()`).
    fn camera_position(partial: f32, position: &mut [f64; 3]);
}
