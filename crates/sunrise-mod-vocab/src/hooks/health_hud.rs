//! The `health_hud` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0056

pub trait Hooks {
    /// Selects the renderer for the player-health HUD rows (hearts, armour, food and air).
    /// `inputs` carries the health, food, air and vehicle state after `player_heart_type`;
    /// `wi`/`hi` give the logical GUI size. The first `Some(HealthHudReplacement)` wins.
    /// The game invokes it on the layer's canvas between `gui_layer_pre` and `gui_layer_post`,
    /// unless Pre hides the layer. A refused renderer falls back to vanilla, not to another
    /// listener. Selecting no renderer uses vanilla's layout even if Pre hides the layer.
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
