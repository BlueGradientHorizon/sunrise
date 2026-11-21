//! The `render_gui_post` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// After all HUD layers, including chat, tab list and any inserted layers, and before
    /// screen drawing (NeoForge's `RenderGuiEvent.Post`). Fires even under F1.
    /// Place a box with `corners.place` (boxes stack away from their edge) and draw it
    /// with `event.draw`.
    /// Use `ClientHost::debug_overlay_visible()` to avoid drawing over the debug overlay.
    /// Not heard when a mod canceled `render_gui_pre`. Client-side.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        version = 2,
        neoforge = "RenderGuiEvent.Post"
    )]
    fn render_gui_post(draw: &mut dyn DrawContext, corners: &mut Corners);
}
