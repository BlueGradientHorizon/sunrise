//! The `render_gui_pre` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Before the in-world HUD is drawn (NeoForge's `RenderGuiEvent.Pre`):
    /// return `Flow::Cancel` to hide vanilla layers (and so `gui_layer_pre` and
    /// `gui_layer_post`), `render_gui_post`, chat and ordinary toasts. Mandatory
    /// mod-error toasts are outside this veto; the player's F1 still hides them.
    /// Fires once a frame in a world, not under F1 or while a server dialog or
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
