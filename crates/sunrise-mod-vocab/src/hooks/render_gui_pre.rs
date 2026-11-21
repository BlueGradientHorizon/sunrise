//! The `render_gui_pre` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Before the in-world HUD is drawn (NeoForge's `RenderGuiEvent.Pre`):
    /// return `Flow::Cancel` to veto the HUD pass, including the status slots and their
    /// layer events. F1 hides the original status bodies but does not veto their slots.
    /// Fires once a frame in a world, including under F1, but not while a server dialog,
    /// the connection screen or a loading screen covers the window.
    /// Client-side.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "cancel",
        out = "canceled",
        neoforge = "RenderGuiEvent.Pre",
        cancellable = true
    )]
    fn render_gui_pre(canceled: &mut bool);
}
