//! The `compute_fov_modifier` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Player FOV scale, once per client tick while the camera is the local
    /// player, after vanilla effects and before easing (skipped when spectating
    /// another entity).
    /// Invalid values or values outside 0.01..=10 revert to base.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "ComputeFovModifierEvent",
        cancellable = false
    )]
    fn compute_fov_modifier(
        player: &mod_api::client::camera::FovPlayer,
        base: f32,
        modifier: &mut f32,
    );
}
