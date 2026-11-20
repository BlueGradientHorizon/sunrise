//! The `render_player_post` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// After a player is drawn (NeoForge's `RenderPlayerEvent.Post`): once a frame for each player
    /// whose `render_player_pre` wasn't canceled, after its `render_living_post`, or right after its
    /// `render_living_pre` when only that one was canceled. `player`, `who` and `draw` are as
    /// `render_player_pre` has them, and what it draws is kept. `model` borrows the same final
    /// `FrameModel` as `render_living_post`, or `None` if no body pose was produced, including
    /// when only the living pre hook canceled. The player's name tag, leash, flames and shadow
    /// are drawn after it, as `render_living_post` says.
    #[hook(
        side = "client",
        contract = "listen",
        neoforge = "RenderPlayerEvent.Post",
        cancellable = false,
        version = 2
    )]
    fn render_player_post();
}
