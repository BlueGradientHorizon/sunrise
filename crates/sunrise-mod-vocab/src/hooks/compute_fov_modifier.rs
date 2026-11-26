//! The `compute_fov_modifier` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Player FOV scale, once per client tick with a player, after vanilla
    /// effects and before zoom and easing.
    /// Invalid values or values outside 0.01..=10 revert to base.
    #[hook(target = "sunrise_client_gameplay::game::native_hooks::compute_fov_modifier::ClientGame<'_>::native_compute_fov_modifier", 
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
