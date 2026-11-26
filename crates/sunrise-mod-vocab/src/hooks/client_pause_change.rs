//! The `client_pause_change` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// NeoForge's `ClientPauseChangeEvent.Pre`: cancel a proposed pause change.
    #[hook(
        target = "sunrise_client_gameplay::game::native_hooks::client_pause_change::ClientGame<'_>::native_client_pause_change_pre",
        dispatch = "flat",
        side = "client",
        contract = "cancel",
        out = "cancel",
        neoforge = "ClientPauseChangeEvent.Pre",
        cancellable = true
    )]
    fn client_pause_change_pre(paused: bool, cancel: &mut bool);
    /// NeoForge's `ClientPauseChangeEvent.Post`: the pause state changed.
    #[hook(
        target = "sunrise_client_gameplay::game::native_hooks::client_pause_change::ClientGame<'_>::native_client_pause_change_post",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "ClientPauseChangeEvent.Post",
        cancellable = false
    )]
    fn client_pause_change_post(paused: bool);
}
