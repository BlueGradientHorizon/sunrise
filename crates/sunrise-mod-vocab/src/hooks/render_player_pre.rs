//! The `render_player_pre` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Before a player is drawn (NeoForge's `RenderPlayerEvent.Pre`): once a frame for each other
    /// player in view and for this window's player in third person (and F5 front view), before its
    /// `render_living_pre`. `player` is its render state, `who` its profile name and whether it is
    /// this window's own player, and `draw` the frame `render_living_pre` gets. Set `canceled` to
    /// skip all of the player's drawing this frame, `render_living_pre`, `render_living_post` and
    /// `render_player_post` included. `event.model` borrows the selected player's hierarchy and
    /// bind pose as a [`crate::FrameModelEdit`]. Look up `model.part("head")` and call
    /// `model.hide(index)` if it exists. That leaves the head and its children out for this draw
    /// after Skin Customization. Living Pre sees the same draw-local edits. Armour, held items
    /// and other independent layers, including the cape, are unaffected.
    #[hook(
        side = "client",
        version = 2,
        contract = "cancel",
        out = "canceled",
        neoforge = "RenderPlayerEvent.Pre",
        cancellable = true
    )]
    fn render_player_pre();
}
