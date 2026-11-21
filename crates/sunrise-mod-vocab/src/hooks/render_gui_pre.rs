//! The `render_gui_pre` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Before the in-world HUD is drawn (NeoForge's `RenderGuiEvent.Pre`):
    /// return `Flow::Cancel` to skip vanilla layers (and so `gui_layer_pre` and
    /// `gui_layer_post`), `render_gui_post`, chat and ordinary toasts. Audio stream
    /// failure notices are outside this veto; the player can still hide them with F1.
    /// Fires once a frame in a world, not under F1 and not while a server dialog or
    /// the connection screen covers the window.
    /// Client-side.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "cancel",
        out = "canceled",
        neoforge = "RenderGuiEvent.Pre",
        cancellable = true,
        version = 2
    )]
    fn render_gui_pre(canceled: &mut bool);
}
