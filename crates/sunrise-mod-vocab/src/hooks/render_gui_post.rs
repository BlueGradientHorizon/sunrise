//! The `render_gui_post` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// After the visible in-world HUD layer sequence finishes and before screens,
    /// toasts, debug overlays and IME draw (NeoForge's `RenderGuiEvent.Post`,
    /// `GuiLayerManager.render`). Use `event.draw` to draw in GUI pixels, and
    /// `ClientHost::hud_layer_bounds` to read the clipped bounds of earlier HUD
    /// layers (for example `"minecraft:effects"` or `"minecraft:scoreboard_sidebar"`).
    /// Fires even under F1 and before Options over a world. Skipped when there is
    /// no in-world HUD pass or when `render_gui_pre` canceled.
    /// Client-side.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        cancellable = false,
        version = 2,
        neoforge = "RenderGuiEvent.Post"
    )]
    fn render_gui_post(draw: &mut dyn DrawContext);
}
