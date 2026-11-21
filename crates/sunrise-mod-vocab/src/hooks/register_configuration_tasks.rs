//! Tasks registered before admitting a connecting player.
pub trait Hooks {
    /// Append tasks for this connecting profile, on the server thread after the
    /// built-in configuration steps. Runs for every peer, including vanilla.
    /// The profile has no player handle yet. Use `event.connection.has_channel`
    /// to check what the peer negotiated and `event.register(task)` to give this
    /// connection an owned task in FIFO order. A panicking listener discards its
    /// tasks and journaled writes; other listeners still configure the same peer.
    /// Only that registration callback is disabled for later peers.
    /// Tasks start in registration order and can finish, wait or fail this peer.
    /// Delayed tasks complete via `IPayloadContext::finish_configuration_task`
    /// with their task type from either a server main-thread or network-thread
    /// configuration payload handler.
    /// There is no task timeout: answering keep-alives can hold configuration
    /// indefinitely. A silent or disconnected connection still ends normally.
    #[hook(
        dispatch = "flat",
        side = "server",
        contract = "listen",
        version = 3,
        neoforge = "RegisterConfigurationTasksEvent"
    )]
    fn register_configuration_tasks(
        connection: mod_api::ConfigurationConnection,
        player: u128,
        name: &str,
        registration: &mut mod_api::__private::ConfigurationTaskRegistration,
    );
}
