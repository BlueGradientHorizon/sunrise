//! The `health_hud` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0056

pub trait Hooks {
    /// Selects the renderer for the player-health HUD rows (hearts, armour, food and air).
    /// `inputs` carries the health, food, air and vehicle state after `player_heart_type`;
    /// `wi` and `hi` are the logical GUI size. Return `None` to defer, or return a
    /// [`HealthHudReplacement`](crate::HealthHudReplacement); the first answer wins. The game
    /// draws it between `gui_layer_pre` and `gui_layer_post` unless Pre hides the layer. A
    /// refused renderer falls back to vanilla, not to another listener. A successful empty
    /// draw replaces vanilla. With no answer, vanilla lays out the rows even if Pre hides them.
    #[hook(
        side = "client",
        contract = "replace",
        out = "replacement",
        answered_by = "mod",
        version = 2,
        neoforge = "RenderGuiLayerEvent.Pre"
    )]
    fn health_hud();
}
