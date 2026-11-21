//! The `render_gui_pre` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Before the in-world HUD is drawn (NeoForge's `RenderGuiEvent.Pre`):
    /// Return `Flow::Cancel` to veto the HUD pass, including the status slots, their
    /// layer events, `render_gui_post`, chat and ordinary toasts. Mandatory mod-error
    /// toasts are outside the veto; the player's F1 still hides them.
    /// Fires once a frame in a world, including under F1, but not while a server dialog,
    /// the connection screen or a loading screen covers the window. F1 hides the original
    /// status bodies, not the slots' installed bodies or events.
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
