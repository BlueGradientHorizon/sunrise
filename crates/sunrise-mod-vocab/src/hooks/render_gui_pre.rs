//! The `render_gui_pre` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Before the in-world HUD is drawn (NeoForge's `RenderGuiEvent.Pre`):
    /// return `Flow::Cancel` to skip the HUD layer walk and `render_gui_post`.
    /// F1 suppresses vanilla bodies but does not cancel the layer walk. Pre still runs
    /// under F1; it does not run when the connection overlay replaces the HUD pass.
    /// Client-side.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "cancel",
        out = "canceled",
        version = 2,
        neoforge = "RenderGuiEvent.Pre",
        cancellable = true
    )]
    fn render_gui_pre(canceled: &mut bool);
}
