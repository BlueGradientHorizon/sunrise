//! The `render_player_post` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// After a player is drawn (NeoForge's `RenderPlayerEvent.Post`): once a frame for each player
    /// whose `render_player_pre` wasn't canceled, after its `render_living_post`, or right after its
    /// `render_living_pre` when only that one was canceled. `player`, `who` and `draw` are as
    /// `render_player_pre` has them, and what it draws is kept. `event.model` borrows the same
    /// final [`crate::FrameModel`] as `render_living_post`, read-only for the callback. Its poses
    /// and effective visibility include this draw's animation, Skin Customization and Pre edits.
    /// It is `None` if no body pose was produced, including when only Living Pre canceled. The
    /// player's name tag, leash, flames and shadow are drawn after it, as in `render_living_post`.
    #[hook(
        side = "client",
        contract = "listen",
        neoforge = "RenderPlayerEvent.Post",
        cancellable = false,
        version = 2
    )]
    fn render_player_post();
}
