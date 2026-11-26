//! Client shutdown completion hook.

pub trait Hooks {
    /// The client has finished shutting down, after `client_stopping`. Fires once
    /// on the render thread for an orderly client exit, not for a dedicated server.
    /// NeoForge's `ClientStoppedEvent`.
    #[hook(
        target = "sunrise_client_gameplay::game::native_hooks::client_stopped::ClientGame<'_>::native_client_stopped",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "ClientStoppedEvent",
        cancellable = false
    )]
    fn client_stopped();
}
