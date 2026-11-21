//! The `customize_chat_overlay` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// The original background HUD chat is about to draw (NeoForge's
    /// `CustomizeGuiOverlayEvent.Chat`, in `Hud.extractChat`). `x` and `y` are its bottom-left
    /// corner in GUI pixels (0 and the GUI height minus 40); change them to move the drawing.
    /// `gui_width` and `gui_height` are the scaled GUI size. This does not fire when F1 hides
    /// the original body, a mod skips it, or the chat screen paints its own focused chat.
    /// Client-side.
    #[hook(
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "CustomizeGuiOverlayEvent.Chat",
        version = 2,
        cancellable = false
    )]
    fn customize_chat_overlay(gui_width: i32, gui_height: i32, x: &mut i32, y: &mut i32);
}
