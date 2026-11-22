//! The `render_gui_pre` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Before the in-world HUD layer sequence (NeoForge's `RenderGuiEvent.Pre`).
    /// Fires even under F1 and before Options over a world, but not on the title or
    /// while the connection or level-loading screen replaces the world HUD.
    /// Return `Flow::Cancel` to skip the whole layer sequence, including sleep and
    /// subtitle layers, its layer hooks, and `render_gui_post`. The open screen
    /// still draws. Ordinary toasts remain hidden on cancellation; audio stream
    /// failure notices are outside that veto.
    /// Client-side.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "cancel",
        out = "canceled",
        neoforge = "RenderGuiEvent.Pre",
        cancellable = true,
        version = 3
    )]
    fn render_gui_pre(canceled: &mut bool);
}
